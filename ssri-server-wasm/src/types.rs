use core::marker::PhantomData;
use std::fmt::Debug;

use ckb_jsonrpc_types::{BlockNumber, CellOutput, JsonBytes, Script, Uint64};
use ckb_vm::Bytes;
use core::fmt;
use hex::{FromHex, ToHex};
use serde::{
    de::{Error, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

pub fn serialize<S, T>(data: T, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: ToHex,
{
    let s = format!("0x{}", data.encode_hex::<String>());
    serializer.serialize_str(&s)
}

pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromHex,
    <T as FromHex>::Error: fmt::Display,
{
    struct HexStrVisitor<T>(PhantomData<T>);

    impl<'de, T> Visitor<'de> for HexStrVisitor<T>
    where
        T: FromHex,
        <T as FromHex>::Error: fmt::Display,
    {
        type Value = T;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "a 0x-prefixed hex encoded string")
        }

        fn visit_str<E>(self, data: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if !data.starts_with("0x") {
                Err(Error::custom("expected a 0x-prefixed hex encoded string"))
            } else {
                FromHex::from_hex(&data[2..]).map_err(Error::custom)
            }
        }

        fn visit_borrowed_str<E>(self, data: &'de str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if !data.starts_with("0x") {
                Err(Error::custom("expected a 0x-prefixed hex encoded string"))
            } else {
                FromHex::from_hex(&data[2..]).map_err(Error::custom)
            }
        }
    }

    deserializer.deserialize_str(HexStrVisitor(PhantomData))
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(transparent)]
pub struct Hex {
    #[serde(serialize_with = "serialize", deserialize_with = "deserialize")]
    pub hex: Vec<u8>,
}

impl Debug for Hex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", self.hex.encode_hex::<String>())
    }
}

impl From<Bytes> for Hex {
    fn from(value: Bytes) -> Self {
        Self {
            hex: value.to_vec(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CellOutputWithData {
    pub cell_output: CellOutput,
    pub hex_data: Option<Hex>,
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchKey {
    pub script: Script,
    pub script_type: ScriptType,
    pub script_search_mode: Option<SearchMode>,
    pub filter: Option<SearchKeyFilter>,
    pub with_data: Option<bool>,
    pub group_by_transaction: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    // search with prefix
    Prefix,
    // search with exact match
    Exact,
    // search with partial match
    Partial,
}

impl Default for SearchMode {
    fn default() -> Self {
        Self::Prefix
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct SearchKeyFilter {
    pub script: Option<Script>,
    pub script_len_range: Option<[Uint64; 2]>,
    pub output_data: Option<JsonBytes>,
    pub output_data_filter_mode: Option<SearchMode>,
    pub output_data_len_range: Option<[Uint64; 2]>,
    pub output_capacity_range: Option<[Uint64; 2]>,
    pub block_range: Option<[BlockNumber; 2]>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ScriptType {
    Lock,
    Type,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    Desc,
    Asc,
}