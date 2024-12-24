use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use ckb_jsonrpc_types::{CellWithStatus, JsonBytes, OutPoint, Uint32};
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::from_value;
use wasm_bindgen::{JsCast, JsError, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response, Url};

use crate::error::Error;

#[derive(Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: String,
    method: String,
    params: T,
    id: u64,
}

#[derive(Deserialize)]
struct JsonRpcResponse<T> {
    jsonrpc: String,
    result: T,
    id: u64,
}

#[derive(Clone)]
pub struct WebSysClient {
    url: Url,
    id: Arc<AtomicU64>,
}

impl WebSysClient {
    pub fn new(ckb_uri: &str) -> Self {
        let uri = Url::new(ckb_uri).expect("ckb uri, e.g. \"http://127.0.0.1:8114\"");
        Self {
            url: uri,
            id: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl WebSysClient {
    pub async fn call_rpc<T, R>(url: &str, method: &str, params: T) -> Result<R, JsError>
    where
        T: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: 1,
        };

        let opts = RequestInit::new();
        opts.set_method("POST");
        opts.set_mode(RequestMode::Cors);
        opts.set_body(&JsValue::from_str(
            &serde_json::to_string(&request).map_err(|e| JsError::new(&e.to_string()))?,
        ));

        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|e| JsError::new(&e.as_string().unwrap_or_default()))?;
        request
            .headers()
            .set("Content-Type", "application/json")
            .map_err(|e| JsError::new(&e.as_string().unwrap_or_default()))?;

        let window = web_sys::window().ok_or_else(|| JsError::new("No window found"))?;
        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| JsError::new(&e.as_string().unwrap_or_default()))?;
        let resp: Response = resp_value
            .dyn_into()
            .map_err(|e| JsError::new(&e.as_string().unwrap_or_default()))?;

        let json = JsFuture::from(
            resp.json()
                .map_err(|e| JsError::new(&e.as_string().unwrap_or_default()))?,
        )
        .await
        .map_err(|e| JsError::new(&e.as_string().unwrap_or_default()))?;
        let response: JsonRpcResponse<R> =
            from_value(json).map_err(|e| JsError::new(&e.to_string()))?;

        Ok(response.result)
    }
    pub fn get_live_cell(
        &self,
        out_point: &OutPoint,
        with_data: bool,
    ) -> JsonRpcResponse<CellWithStatus> {
        jsonrpc!("get_live_cell", self, CellWithStatus, out_point, with_data).boxed()
    }

    pub fn get_cells(
        &self,
        search_key: SearchKey,
        limit: u32,
        cursor: Option<JsonBytes>,
    ) -> Result<Pagination<Cell>, ckb_sdk::RpcError> {
        let order = Order::Asc;
        let limit = Uint32::from(limit);

        // jsonrpc!(
        //     "get_cells",
        //     self,
        //     Pagination<Cell>,
        //     search_key,
        //     order,
        //     limit,
        //     cursor,
        // )
        // .boxed()
    }

    pub fn get_live_cell_ckb(
        &self,
        out_point: &OutPoint,
        with_data: bool,
    ) -> Result<CellWithStatus, ckb_sdk::rpc::RpcError> {
    }
}
