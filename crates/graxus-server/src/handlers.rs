use graxus_agent_api::context::ContextEngine;
use serde_json::json;

use crate::rpc::{RpcRequest, RpcResponse};
use crate::state::ServerState;

/// Dispatch an RPC request to the appropriate handler.
pub async fn handle_request(state: &mut ServerState, request: RpcRequest) -> RpcResponse {
    match request.method.as_str() {
        "ping" => RpcResponse::success(request.id, json!("pong")),

        "status" => handle_status(state, request.id),

        "context" => handle_context(state, request.params, request.id),

        "file_context" => handle_file_context(state, request.params, request.id),

        "symbol_context" => handle_symbol_context(state, request.params, request.id),

        "update" => handle_update(state, request.id),

        _ => RpcResponse::error(request.id, -32601, "Method not found"),
    }
}

fn handle_status(state: &ServerState, id: Option<serde_json::Value>) -> RpcResponse {
    let result = json!({
        "project": state.config.project.name,
        "root": state.root.display().to_string(),
        "has_doc_graph": state.doc_graph.is_some(),
        "has_code_graph": state.code_graph.is_some(),
        "has_bridge": state.bridge.is_some(),
        "doc_nodes": state.doc_graph.as_ref().map(|g| g.nodes.len()).unwrap_or(0),
        "doc_edges": state.doc_graph.as_ref().map(|g| g.edges.len()).unwrap_or(0),
        "code_files": state.code_graph.as_ref().map(|g| g.files.len()).unwrap_or(0),
        "symbols": state.code_graph.as_ref().map(|g| g.symbols.len()).unwrap_or(0),
        "imports": state.code_graph.as_ref().map(|g| g.imports.len()).unwrap_or(0),
        "calls": state.code_graph.as_ref().map(|g| g.calls.len()).unwrap_or(0),
        "bridge_edges": state.bridge.as_ref().map(|b| b.len()).unwrap_or(0),
    });
    RpcResponse::success(id, result)
}

fn handle_context(
    state: &ServerState,
    params: Option<serde_json::Value>,
    id: Option<serde_json::Value>,
) -> RpcResponse {
    let query = match params.as_ref().and_then(|p| p.get("query")).and_then(|q| q.as_str()) {
        Some(q) => q,
        None => return RpcResponse::error(id, -32602, "Missing 'query' param"),
    };

    match build_context_engine(state) {
        Some(engine) => {
            let ctx = engine.query(query);
            match serde_json::to_value(&ctx) {
                Ok(v) => RpcResponse::success(id, v),
                Err(e) => RpcResponse::error(id, -32603, &format!("Serialization error: {}", e)),
            }
        }
        None => RpcResponse::error(id, -32000, "No indexed data available. Run `graxus index` first."),
    }
}

fn handle_file_context(
    state: &ServerState,
    params: Option<serde_json::Value>,
    id: Option<serde_json::Value>,
) -> RpcResponse {
    let file = match params.as_ref().and_then(|p| p.get("file")).and_then(|q| q.as_str()) {
        Some(f) => f,
        None => return RpcResponse::error(id, -32602, "Missing 'file' param"),
    };

    match build_context_engine(state) {
        Some(engine) => {
            let ctx = engine.file_context(file);
            match serde_json::to_value(&ctx) {
                Ok(v) => RpcResponse::success(id, v),
                Err(e) => RpcResponse::error(id, -32603, &format!("Serialization error: {}", e)),
            }
        }
        None => RpcResponse::error(id, -32000, "No indexed data available. Run `graxus index` first."),
    }
}

fn handle_symbol_context(
    state: &ServerState,
    params: Option<serde_json::Value>,
    id: Option<serde_json::Value>,
) -> RpcResponse {
    let symbol = match params.as_ref().and_then(|p| p.get("symbol")).and_then(|q| q.as_str()) {
        Some(s) => s,
        None => return RpcResponse::error(id, -32602, "Missing 'symbol' param"),
    };

    match build_context_engine(state) {
        Some(engine) => {
            let ctx = engine.symbol_context(symbol);
            match serde_json::to_value(&ctx) {
                Ok(v) => RpcResponse::success(id, v),
                Err(e) => RpcResponse::error(id, -32603, &format!("Serialization error: {}", e)),
            }
        }
        None => RpcResponse::error(id, -32000, "No indexed data available. Run `graxus index` first."),
    }
}

fn handle_update(state: &mut ServerState, id: Option<serde_json::Value>) -> RpcResponse {
    match state.reload() {
        Ok(()) => {
            let result = json!({
                "status": "reloaded",
                "doc_nodes": state.doc_graph.as_ref().map(|g| g.nodes.len()).unwrap_or(0),
                "symbols": state.code_graph.as_ref().map(|g| g.symbols.len()).unwrap_or(0),
            });
            RpcResponse::success(id, result)
        }
        Err(e) => RpcResponse::error(id, -32603, &format!("Reload failed: {}", e)),
    }
}

fn build_context_engine(state: &ServerState) -> Option<ContextEngine> {
    let doc_graph = state.doc_graph.clone()?;
    let code_graph = state.code_graph.clone()?;
    Some(ContextEngine::build(doc_graph, code_graph).ok()?)
}
