pub mod handlers;
pub mod rpc;
pub mod state;

use anyhow::Result;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};

use rpc::RpcRequest;

/// Run a JSON-RPC server on stdio (stdin/stdout).
pub async fn run_stdio(root: PathBuf) -> Result<()> {
    let mut state = state::ServerState::load(root)?;
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    tracing::info!("Graxus server started on stdio");

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            tracing::info!("EOF received, shutting down");
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: RpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let resp = rpc::RpcResponse::error(None, -32700, &format!("Parse error: {}", e));
                println!("{}", serde_json::to_string(&resp)?);
                continue;
            }
        };

        let response = handlers::handle_request(&mut state, request).await;
        println!("{}", serde_json::to_string(&response)?);
    }

    Ok(())
}
