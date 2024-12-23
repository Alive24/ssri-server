import * as wasmModule from "./pkg/ssri-server";
import wasm from "./pkg/ssri-server_bg.wasm";
wasmModule.initSync({ module: wasm });

export default wasmModule;
