pub mod handlers;
pub mod lsp;
pub mod rpc;
pub mod state;

use anyhow::Result;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use rpc::RpcRequest;

/// Maximum request payload size in bytes (1 MB).
const MAX_REQUEST_SIZE: usize = 1_048_576;

/// Default rate limit: maximum requests per sliding window.
const DEFAULT_RATE_LIMIT: u64 = 100;

/// Sliding-window rate limiter that tracks request timestamps.
///
/// Uses a one-minute window. Tokens are counted as timestamps within
/// the window; once the count reaches `max_requests`, further requests
/// are rejected until older timestamps expire.
pub struct RateLimiter {
    /// Timestamps of recent requests within the current window.
    timestamps: VecDeque<Instant>,
    /// Maximum number of requests allowed per window.
    max_requests: u64,
    /// Duration of the sliding window.
    window: std::time::Duration,
}

impl RateLimiter {
    /// Create a new rate limiter allowing `max_requests` per minute.
    pub fn new(max_requests: u64) -> Self {
        Self {
            timestamps: VecDeque::new(),
            max_requests,
            window: std::time::Duration::from_secs(60),
        }
    }

    /// Try to admit a request. Returns `true` if allowed, `false` if rate-limited.
    pub fn check(&mut self) -> bool {
        let now = Instant::now();
        // Evict timestamps outside the window.
        while let Some(&front) = self.timestamps.front() {
            if now.duration_since(front) > self.window {
                self.timestamps.pop_front();
            } else {
                break;
            }
        }
        if self.timestamps.len() as u64 >= self.max_requests {
            false
        } else {
            self.timestamps.push_back(now);
            true
        }
    }
}

/// Validate the `authorization` field in `raw_json` against the expected API key.
///
/// Returns `true` if the key matches, `false` otherwise. The function
/// extracts the `"authorization"` field from the raw JSON and compares
/// it using constant-time comparison to prevent timing attacks.
fn check_auth(raw_json: &str, expected_key: &str) -> bool {
    // Parse just enough to extract the authorization field.
    match serde_json::from_str::<serde_json::Value>(raw_json) {
        Ok(val) => val
            .get("authorization")
            .and_then(|v| v.as_str())
            .map(|provided| {
                use subtle::ConstantTimeEq;
                provided.as_bytes().ct_eq(expected_key.as_bytes()).into()
            })
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// Run a JSON-RPC server on stdio (stdin/stdout).
///
/// Reads newline-delimited JSON-RPC requests from stdin and writes
/// responses to stdout. Supports the following environment variables:
///
/// - `GRAXUS_API_KEY` — if set, every request must include a matching
///   `"authorization"` field (string equality, no hashing).
/// - `GRAXUS_RATE_LIMIT` — maximum requests per minute (default 100).
///
/// Requests larger than 1 MB are rejected before parsing.
pub async fn run_stdio(root: PathBuf) -> Result<()> {
    let mut state = state::ServerState::load(root)?;
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    // Authentication key (None = no auth required).
    let api_key = std::env::var("GRAXUS_API_KEY").ok();

    if api_key.is_none() {
        tracing::warn!(
            "Server running without authentication. Set GRAXUS_API_KEY for production use."
        );
    }

    // Rate limiter configuration.
    let rate_limit = std::env::var("GRAXUS_RATE_LIMIT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_RATE_LIMIT);
    let mut rate_limiter = RateLimiter::new(rate_limit);

    tracing::info!("Graxus server started on stdio");

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            tracing::info!("EOF received, shutting down");
            break;
        }

        // --- Input size limit (1 MB) ---
        if line.len() > MAX_REQUEST_SIZE {
            let resp = rpc::RpcResponse::error(None, -32600, "Request too large (max 1MB allowed)");
            println!("{}", serde_json::to_string(&resp)?);
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // --- Parse JSON-RPC request ---
        let request: RpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let resp = rpc::RpcResponse::error(None, -32700, &format!("Parse error: {}", e));
                println!("{}", serde_json::to_string(&resp)?);
                continue;
            }
        };

        // --- Authentication ---
        if let Some(ref key) = api_key {
            if !check_auth(trimmed, key) {
                let resp = rpc::RpcResponse::error(
                    request.id,
                    -32600,
                    "Unauthorized: invalid or missing API key",
                );
                println!("{}", serde_json::to_string(&resp)?);
                continue;
            }
        }

        // --- Rate limiting ---
        if !rate_limiter.check() {
            let resp = rpc::RpcResponse::error(request.id, -32029, "Rate limit exceeded");
            println!("{}", serde_json::to_string(&resp)?);
            continue;
        }

        // --- Dispatch ---
        let response = handlers::handle_request(&mut state, request).await;
        println!("{}", serde_json::to_string(&response)?);
    }

    Ok(())
}

/// Run the LSP server on stdio using Content-Length framed messages.
///
/// Reads LSP requests from stdin and writes responses to stdout.
/// Uses the standard LSP `Content-Length` header protocol instead of
/// the newline-delimited JSON-RPC used by `run_stdio`.
pub async fn run_lsp(root: PathBuf) -> Result<()> {
    let server = lsp::LspServer::load(root)?;
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();

    tracing::info!("Graxus LSP server started on stdio");

    loop {
        // Read Content-Length header
        let content_length = match read_content_length(&mut stdin).await? {
            Some(len) => len,
            None => {
                tracing::info!("EOF received, shutting down LSP server");
                break;
            }
        };

        // Read the message body
        let mut body = vec![0u8; content_length];
        stdin.read_exact(&mut body).await?;

        let raw = String::from_utf8(body)?;
        let msg = match lsp::parse_lsp_message(&raw) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("Failed to parse LSP message: {}", e);
                continue;
            }
        };

        // Check for shutdown before dispatching
        let is_exit = msg.method.as_deref() == Some("exit");

        let response = server.handle(&msg);
        let serialized = lsp::serialize_lsp_message(&response)?;
        stdout.write_all(serialized.as_bytes()).await?;
        stdout.flush().await?;

        if is_exit {
            tracing::info!("Exit requested, shutting down LSP server");
            break;
        }
    }

    Ok(())
}

/// Read the Content-Length header from the input stream.
/// Returns the content length, or None on EOF.
async fn read_content_length(reader: &mut tokio::io::Stdin) -> Result<Option<usize>> {
    let mut header_buf = Vec::new();
    let mut byte = [0u8; 1];

    loop {
        let n = reader.read(&mut byte).await?;
        if n == 0 {
            return if header_buf.is_empty() {
                Ok(None)
            } else {
                Err(anyhow::anyhow!("Unexpected EOF in header"))
            };
        }
        header_buf.push(byte[0]);

        // Check if we have a complete header (ends with \r\n\r\n)
        if header_buf.ends_with(b"\r\n\r\n") {
            break;
        }

        // Guard against malformed input eating all memory
        if header_buf.len() > 256 {
            return Err(anyhow::anyhow!("Header too long (>256 bytes)"));
        }
    }

    let header = String::from_utf8(header_buf)?;
    for line in header.lines() {
        if let Some(val) = line.strip_prefix("Content-Length:") {
            let len = val.trim().parse::<usize>()?;
            return Ok(Some(len));
        }
    }

    Err(anyhow::anyhow!("Missing Content-Length header"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Auth tests ----

    #[test]
    fn auth_matching_key() {
        let raw = r#"{"method":"ping","authorization":"my-secret"}"#;
        assert!(check_auth(raw, "my-secret"));
    }

    #[test]
    fn auth_wrong_key() {
        let raw = r#"{"method":"ping","authorization":"wrong"}"#;
        assert!(!check_auth(raw, "my-secret"));
    }

    #[test]
    fn auth_missing_field() {
        let raw = r#"{"method":"ping"}"#;
        assert!(!check_auth(raw, "my-secret"));
    }

    #[test]
    fn auth_empty_key() {
        let raw = r#"{"method":"ping","authorization":""}"#;
        assert!(!check_auth(raw, "my-secret"));
    }

    #[test]
    fn auth_invalid_json() {
        assert!(!check_auth("not json", "key"));
    }

    // ---- Rate limiter tests ----

    #[test]
    fn rate_limiter_allows_within_limit() {
        let mut limiter = RateLimiter::new(5);
        for _ in 0..5 {
            assert!(limiter.check());
        }
    }

    #[test]
    fn rate_limiter_rejects_over_limit() {
        let mut limiter = RateLimiter::new(3);
        assert!(limiter.check());
        assert!(limiter.check());
        assert!(limiter.check());
        assert!(!limiter.check());
    }

    #[test]
    fn rate_limiter_default_capacity() {
        let mut limiter = RateLimiter::new(DEFAULT_RATE_LIMIT);
        // Should allow at least DEFAULT_RATE_LIMIT requests.
        for _ in 0..DEFAULT_RATE_LIMIT {
            assert!(limiter.check());
        }
        assert!(!limiter.check());
    }

    // ---- Input size tests ----

    #[test]
    fn max_request_size_is_one_mb() {
        assert_eq!(MAX_REQUEST_SIZE, 1_048_576);
    }

    // ---- Integration: RPC + auth + rate-limit message format ----

    #[test]
    fn rate_limit_error_code() {
        let resp =
            rpc::RpcResponse::error(Some(serde_json::json!(1)), -32029, "Rate limit exceeded");
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("-32029"));
        assert!(s.contains("Rate limit exceeded"));
    }

    #[test]
    fn unauthorized_error_code() {
        let resp = rpc::RpcResponse::error(
            Some(serde_json::json!(1)),
            -32600,
            "Unauthorized: invalid or missing API key",
        );
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("-32600"));
        assert!(s.contains("Unauthorized"));
    }

    #[test]
    fn request_too_large_error_code() {
        let resp = rpc::RpcResponse::error(None, -32600, "Request too large (max 1MB allowed)");
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("-32600"));
        assert!(s.contains("too large"));
    }
}
