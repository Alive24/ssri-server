use wasm_bindgen::prelude::*;

use ckb_jsonrpc_types::{OutPoint, Script, TransactionView};
use ckb_types::H256;

mod error;
mod network;
mod ssri_vm;
mod types;

use error::Error;
use types::{CellOutputWithData, Hex};

use ssri_vm::execute_riscv_binary;


async fn run_script(
    &self,
    tx_hash: H256,
    index: u32,
    args: Vec<Hex>,
    script: Option<Script>,
    cell: Option<CellOutputWithData>,
    tx: Option<TransactionView>,
) -> Result<Option<Hex>, ErrorObjectOwned> {
    let ssri_cell = self
        .rpc
        .get_live_cell(
            &OutPoint {
                tx_hash: tx_hash.0.into(),
                index: index.into(),
            },
            true,
        )
        .await?;

    tracing::info!("Running script on {tx_hash}:{index} with args {args:?}");

    let ssri_binary = ssri_cell
        .cell
        .ok_or(Error::InvalidRequest("Cell not found"))?
        .data
        .ok_or(Error::InvalidRequest("Cell doesn't have data"))?
        .content
        .into_bytes();

    let args = args.into_iter().map(|v| v.hex.into()).collect();
    let script = script.map(Into::into);
    let cell = cell.map(Into::into);
    let tx = tx.map(|v| v.inner.into());

    Ok(
        execute_riscv_binary(self.rpc.clone(), ssri_binary, args, script, cell, tx)?
            .map(|v| v.into()),
    )
}

#[wasm_bindgen]
async fn run_script_level_code(
    &self,
    tx_hash: H256,
    index: u32,
    args: Vec<Hex>,
) -> Result<Option<Hex>, ErrorObjectOwned> {
    self.run_script(tx_hash, index, args, None, None, None)
        .await
}

#[wasm_bindgen]
async fn run_script_level_script(
    &self,
    tx_hash: H256,
    index: u32,
    args: Vec<Hex>,
    script: Script,
) -> Result<Option<Hex>, ErrorObjectOwned> {
    println!("script: {:?}", script);
    self.run_script(tx_hash, index, args, Some(script), None, None)
        .await
}

#[wasm_bindgen]
async fn run_script_level_cell(
    &self,
    tx_hash: H256,
    index: u32,
    args: Vec<Hex>,
    cell: CellOutputWithData,
) -> Result<Option<Hex>, ErrorObjectOwned> {
    self.run_script(tx_hash, index, args, None, Some(cell), None)
        .await
}

#[wasm_bindgen]
async fn run_script_level_tx(
    &self,
    tx_hash: H256,
    index: u32,
    args: Vec<Hex>,
    tx: TransactionView,
) -> Result<Option<Hex>, ErrorObjectOwned> {
    self.run_script(tx_hash, index, args, None, None, Some(tx))
        .await
}


// use this only for wasm
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub async fn run_server_main() -> anyhow::Result<()> {
    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .expect("setting default subscriber failed");

    let ckb_rpc = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://testnet.ckbapp.dev/".to_string());
    let server_addr = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "0.0.0.0:9090".to_string());

    Ok(())
}

