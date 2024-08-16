// refer to https://github.com/nervosnetwork/ckb-vm/blob/develop/examples/ckb-vm-runner.rs

use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use ckb_hash::blake2b_256;
use ckb_sdk::traits::CellQueryOptions;
use ckb_types::core::Capacity;
use ckb_types::packed::{CellOutput, OutPoint, Script, Transaction};
use ckb_types::prelude::Entity;
use ckb_vm::cost_model::estimate_cycles;
use ckb_vm::registers::{A0, A1, A2, A3, A4, A5, A7};
use ckb_vm::{Bytes, Memory, Register, SupportMachine, Syscalls};
use hex::encode;

use crate::error::Error;
use crate::rpc_client::RpcClient;
use crate::types::CellOutputWithData;

macro_rules! error {
    ($err:expr) => {{
        let error = $err.to_string();
        #[cfg(test)]
        println!("[ERROR] {error}");
        #[cfg(not(test))]
        jsonrpsee::tracing::error!("{error}");
        ckb_vm::error::Error::Unexpected(error)
    }};
}

macro_rules! output {
    ($machine:ident, $len_addr:ident, $bytes:expr, $addr:ident, $offset:expr, $len:ident) => {
        $machine
            .memory_mut()
            .store64(&$len_addr, &($bytes.len() as u64))?;
        if $len > 0 {
            let begin = $offset as usize;
            let end = ($offset + $len) as usize;
            $machine
                .memory_mut()
                .store_bytes($addr, &$bytes[begin..end])?;
        }
    };
}

#[allow(unused)]
#[repr(u64)]
pub enum Source {
    Input = 1,
    Output = 2,
    CellDep = 3,
    HeaderDep = 4,
    GroupInput = 72_057_594_037_927_937,
    GroupOutput = 72_057_594_037_927_938,
}

#[allow(unused)]
#[repr(u64)]
pub enum CellField {
    Capacity = 0,
    DataHash = 1,
    Lock = 2,
    LockHash = 3,
    Type = 4,
    TypeHash = 5,
    OccupiedCapacity = 6,
}

impl TryFrom<u64> for CellField {
    type Error = ckb_vm::error::Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(CellField::Capacity),
            1 => Ok(CellField::DataHash),
            2 => Ok(CellField::Lock),
            3 => Ok(CellField::LockHash),
            4 => Ok(CellField::Type),
            5 => Ok(CellField::TypeHash),
            6 => Ok(CellField::OccupiedCapacity),
            _ => Err(ckb_vm::error::Error::Unexpected(format!(
                "Invalid cell field {}",
                value
            ))),
        }
    }
}

#[derive(Clone)]
struct Context {
    content: Arc<Mutex<Option<Bytes>>>,
    rpc: RpcClient,
    script: Option<Script>,
    cell: Option<CellOutputWithData>,
    // wait for second phase to do with ckb transaction
    _tx: Option<Transaction>,
}

impl Context {
    pub fn new(
        rpc: RpcClient,
        script: Option<Script>,
        cell: Option<CellOutputWithData>,
        tx: Option<Transaction>,
    ) -> Self {
        Self {
            content: Arc::new(Mutex::new(None)),
            rpc,
            script,
            cell,
            _tx: tx,
        }
    }
}

impl Context {
    fn load_script(
        &self,
        machine: &mut impl SupportMachine<REG = u64>,
    ) -> Result<(), ckb_vm::error::Error> {
        let script = self.script.as_ref().ok_or(error!("Script is missing"))?;
        let addr = machine.registers()[A0].to_u64();
        let len_addr = machine.registers()[A1];
        let len = machine.memory_mut().load64(&len_addr)?;
        let offset = machine.registers()[A2];

        let bytes = script.as_slice().to_vec();
        output!(machine, len_addr, bytes, addr, offset, len);
        Ok(())
    }

    fn load_script_hash(
        &self,
        machine: &mut impl SupportMachine<REG = u64>,
    ) -> Result<(), ckb_vm::error::Error> {
        let script = self.script.as_ref().ok_or(error!("Script is missing"))?;
        let addr = machine.registers()[A0].to_u64();
        let len_addr = machine.registers()[A1];
        let len = machine.memory_mut().load64(&len_addr)?;
        let offset = machine.registers()[A2];

        let bytes = script.calc_script_hash().raw_data().to_vec();
        output!(machine, len_addr, bytes, addr, offset, len);
        Ok(())
    }

    fn load_cell(
        &self,
        machine: &mut impl SupportMachine<REG = u64>,
    ) -> Result<(), ckb_vm::error::Error> {
        let cell = self.cell.clone().ok_or(error!("Cell is missing"))?;
        let addr = machine.registers()[A0].to_u64();
        let len_addr = machine.registers()[A1];
        let len = machine.memory_mut().load64(&len_addr)?;
        let offset = machine.registers()[A2];
        let index = machine.registers()[A3];
        let source = machine.registers()[A4];

        if index != 0 || source != Source::GroupInput as u64 {
            return Err(error!("Invalid index or source"));
        }

        let bytes = CellOutput::from(cell.cell_output).as_slice().to_vec();
        output!(machine, len_addr, bytes, addr, offset, len);
        Ok(())
    }

    fn load_cell_data(
        &self,
        machine: &mut impl SupportMachine<REG = u64>,
    ) -> Result<(), ckb_vm::error::Error> {
        let cell = self.cell.clone().ok_or(error!("Cell is missing"))?;
        let addr = machine.registers()[A0].to_u64();
        let len_addr = machine.registers()[A1];
        let len = machine.memory_mut().load64(&len_addr)?;
        let offset = machine.registers()[A2];
        let index = machine.registers()[A3];
        let source = machine.registers()[A4];

        if index != 0 || source != Source::GroupInput as u64 {
            return Err(error!("Invalid index or source"));
        }

        let bytes = cell.hex_data.ok_or(error!("Cell data is missing"))?.hex;
        output!(machine, len_addr, bytes, addr, offset, len);
        Ok(())
    }

    fn load_cell_by_field(
        &self,
        machine: &mut impl SupportMachine<REG = u64>,
    ) -> Result<(), ckb_vm::error::Error> {
        let cell = self.cell.clone().ok_or(error!("Cell is missing"))?;
        let addr = machine.registers()[A0].to_u64();
        let len_addr = machine.registers()[A1];
        let len = machine.memory_mut().load64(&len_addr)?;
        let offset = machine.registers()[A2];
        let index = machine.registers()[A3];
        let source = machine.registers()[A4];
        let field = machine.registers()[A5];

        if index != 0 || source != Source::GroupInput as u64 {
            return Err(error!("Invalid index or source"));
        }

        let bytes = match field.try_into()? {
            CellField::DataHash => blake2b_256(
                hex::decode(cell.hex_data.ok_or(error!("Cell data is missing"))?.hex)
                    .map_err(|_| error!("Invalid hexed cell data "))?,
            )
            .to_vec(),
            CellField::Lock => Script::from(cell.cell_output.lock).as_slice().to_vec(),
            CellField::Type => cell
                .cell_output
                .type_
                .map(|type_| Script::from(type_).as_slice().to_vec())
                .unwrap_or_default(),
            CellField::LockHash => Script::from(cell.cell_output.lock)
                .calc_script_hash()
                .raw_data()
                .to_vec(),
            CellField::TypeHash => cell
                .cell_output
                .type_
                .map(|type_| Script::from(type_).calc_script_hash().raw_data().to_vec())
                .unwrap_or_default(),
            CellField::Capacity => u64::from(cell.cell_output.capacity).to_le_bytes().to_vec(),
            CellField::OccupiedCapacity => {
                let data_len = cell
                    .hex_data
                    .ok_or(error!("Cell data is missing"))?
                    .hex
                    .len()
                    / 2;
                CellOutput::from(cell.cell_output)
                    .occupied_capacity(Capacity::bytes(data_len).unwrap())
                    .unwrap()
                    .as_u64()
                    .to_le_bytes()
                    .to_vec()
            }
        };

        output!(machine, len_addr, bytes, addr, offset, len);
        Ok(())
    }

    fn find_out_point_by_type(
        &self,
        machine: &mut impl SupportMachine<REG = u64>,
    ) -> Result<(), ckb_vm::error::Error> {
        let addr = machine.registers()[A0].to_u64();
        let len_addr = machine.registers()[A1];
        let len = machine.memory_mut().load64(&len_addr)?;
        let script_addr = machine.registers()[A2];
        let script_len = machine.registers()[A3];

        let script = Script::from_slice(&machine.memory_mut().load_bytes(script_addr, script_len)?)
            .map_err(|_| error!("Invalid type script"))?;

        let rpc = self.rpc.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let cell = rt
                .block_on(rpc.get_cells(CellQueryOptions::new_type(script).into(), 1, None))
                .map_err(|err| error!(err))
                .map(|v| v.objects.into_iter().next());
            tx.send(cell).unwrap();
        });

        let Some(cell) = rx.recv().unwrap()? else {
            return Err(error!("Cell not found"));
        };

        let out_point = OutPoint::from(cell.out_point);
        output!(machine, len_addr, out_point.as_slice(), addr, 0, len);
        Ok(())
    }

    fn find_cell_by_out_point(
        &self,
        machine: &mut impl SupportMachine<REG = u64>,
    ) -> Result<(), ckb_vm::error::Error> {
        let addr = machine.registers()[A0].to_u64();
        let len_addr = machine.registers()[A1];
        let len = machine.memory_mut().load64(&len_addr)?;
        let outpoint_addr = machine.registers()[A2];

        let out_point = OutPoint::from_slice(
            &machine
                .memory_mut()
                .load_bytes(outpoint_addr, OutPoint::TOTAL_SIZE as u64)?,
        )
        .map_err(|_| error!("Invalid type script"))?;

        let rpc = self.rpc.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let cell = rt
                .block_on(rpc.get_live_cell(&out_point.into(), false))
                .map_err(|err| error!(err))
                .map(|v| v.cell);
            tx.send(cell).unwrap();
        });

        let cell: CellOutput = rx
            .recv()
            .unwrap()?
            .ok_or(error!("Cell not found"))?
            .output
            .into();
        output!(machine, len_addr, cell.as_slice(), addr, 0, len);
        Ok(())
    }

    fn find_cell_data_by_out_point(
        &self,
        machine: &mut impl SupportMachine<REG = u64>,
    ) -> Result<(), ckb_vm::error::Error> {
        let addr = machine.registers()[A0].to_u64();
        let len_addr = machine.registers()[A1];
        let len = machine.memory_mut().load64(&len_addr)?;
        let outpoint_addr = machine.registers()[A2];

        let out_point = OutPoint::from_slice(
            &machine
                .memory_mut()
                .load_bytes(outpoint_addr, OutPoint::TOTAL_SIZE as u64)?,
        )
        .map_err(|_| error!("Invalid type script"))?;

        let rpc = self.rpc.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let cell = rt
                .block_on(rpc.get_live_cell(&out_point.into(), true))
                .map_err(|err| error!(err))
                .map(|v| v.cell);
            tx.send(cell).unwrap();
        });

        let data = rx
            .recv()
            .unwrap()?
            .ok_or(error!("Cell not found"))?
            .data
            .unwrap();

        output!(machine, len_addr, data.content.as_bytes(), addr, 0, len);
        Ok(())
    }
}

impl<M: SupportMachine<REG = u64>> Syscalls<M> for Context {
    fn initialize(&mut self, _machine: &mut M) -> Result<(), ckb_vm::error::Error> {
        Ok(())
    }

    fn ecall(&mut self, machine: &mut M) -> Result<bool, ckb_vm::error::Error> {
        match machine.registers()[A7].to_u64() {
            // version - code
            2041 => machine.set_register(A0, u64::MAX),

            // load_script - script
            2052 => self.load_script(machine)?,
            // load_script_hash - script
            2061 => self.load_script_hash(machine)?,
            // load_cell - cell
            2071 => self.load_cell(machine)?,
            // load_cell_data - cell
            2091 => self.load_cell_data(machine)?,
            // load_cell_by_field - cell
            2081 => self.load_cell_by_field(machine)?,
            // find_out_point_by_type - code
            2277 => self.find_out_point_by_type(machine)?,
            // find_cell_by_out_point - code
            2287 => self.find_cell_by_out_point(machine)?,
            // find_cell_data_by_out_point - code
            2297 => self.find_cell_data_by_out_point(machine)?,

            // set_content - code
            2103 => {
                let addr = machine.registers()[A0].to_u64();
                let len = machine.registers()[A1];
                let len = machine.memory_mut().load64(&len)?;

                *self.content.clone().lock().unwrap() =
                    Some(machine.memory_mut().load_bytes(addr, len)?);
            }
            // debug - code
            2177 => {
                let mut addr = machine.registers()[A0];
                let mut buffer = Vec::new();

                loop {
                    let byte = machine.memory_mut().load8(&addr)?.to_u8();
                    if byte == 0 {
                        break;
                    }
                    buffer.push(byte);
                    addr += 1;
                }

                println!("{}", String::from_utf8(buffer).unwrap());
            }
            _ => return Ok(false),
        };

        Ok(true)
    }
}

pub fn execute_riscv_binary(
    rpc: RpcClient,
    code: Bytes,
    args: Vec<Bytes>,
    script: Option<Script>,
    cell: Option<CellOutputWithData>,
    tx: Option<Transaction>,
) -> Result<Option<Bytes>, Error> {
    let context = Context::new(rpc, script, cell, tx);

    let asm_core = ckb_vm::machine::asm::AsmCoreMachine::new(
        ckb_vm::ISA_IMC | ckb_vm::ISA_B | ckb_vm::ISA_MOP | ckb_vm::ISA_A,
        ckb_vm::machine::VERSION2,
        u64::MAX,
    );
    let core = ckb_vm::DefaultMachineBuilder::new(asm_core)
        .instruction_cycle_func(Box::new(estimate_cycles))
        .syscall(Box::new(context.clone()))
        .build();
    let mut machine = ckb_vm::machine::asm::AsmMachine::new(core);

    let args = args
        .into_iter()
        .map(|arg| Bytes::copy_from_slice(encode(arg).as_bytes()))
        .collect::<Vec<Bytes>>();
    machine
        .load_program(&code, &args)
        .map_err(|err| Error::Vm(format!("Failed to load program: {err}")))?;
    let error_code = machine
        .run()
        .map_err(|err| Error::Vm(format!("Failed to run program: {err}")))?;
    if error_code != 0 {
        return Err(Error::Script(error_code));
    }

    let result = context.content.lock().unwrap().clone();
    Ok(result)
}
