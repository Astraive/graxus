use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 request.
///
/// Supports an optional `authorization` field for API key authentication.
/// When the server's `GRAXUS_API_KEY` environment variable is set,
/// every request must include a matching `authorization` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    /// Request identifier (used to correlate responses).
    pub id: Option<serde_json::Value>,
    /// The RPC method name to invoke.
    pub method: String,
    /// Optional method parameters.
    pub params: Option<serde_json::Value>,
    /// Optional API key for authentication (matches `GRAXUS_API_KEY` env var).
    #[serde(default, skip_serializing)]
    pub authorization: Option<String>,
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    /// Request identifier echoed from the request.
    pub id: Option<serde_json::Value>,
    /// Successful result value (mutually exclusive with `error`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error object (mutually exclusive with `result`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    /// Numeric error code.
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
}

impl RpcResponse {
    /// Create a successful response with the given result value.
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response with the given code and message.
    pub fn error(id: Option<serde_json::Value>, code: i32, message: &str) -> Self {
        Self {
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_request_with_all_fields() {
        let json = r#"{"id":1,"method":"ping","params":{"x":1},"authorization":"secret"}"#;
        let req: RpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "ping");
        assert_eq!(req.id, Some(json!(1)));
        assert_eq!(req.params, Some(json!({"x": 1})));
        assert_eq!(req.authorization, Some("secret".to_string()));
    }

    #[test]
    fn parse_request_without_optional_fields() {
        let json = r#"{"method":"status"}"#;
        let req: RpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "status");
        assert_eq!(req.id, None);
        assert_eq!(req.params, None);
        assert_eq!(req.authorization, None);
    }

    #[test]
    fn parse_request_with_null_id() {
        let json = r#"{"id":null,"method":"ping"}"#;
        let req: RpcRequest = serde_json::from_str(json).unwrap();
        // serde deserializes JSON null into None for Option<Value>
        assert_eq!(req.id, None);
    }

    #[test]
    fn success_response_serialization() {
        let resp = RpcResponse::success(Some(json!(1)), json!("pong"));
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"result\""));
        assert!(s.contains("pong"));
        assert!(!s.contains("\"error\""));
    }

    #[test]
    fn error_response_serialization() {
        let resp = RpcResponse::error(Some(json!(1)), -32601, "Method not found");
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"error\""));
        assert!(s.contains("-32601"));
        assert!(s.contains("Method not found"));
        assert!(!s.contains("\"result\""));
    }

    #[test]
    fn error_response_with_null_id() {
        let resp = RpcResponse::error(None, -32700, "Parse error");
        let s = serde_json::to_string(&resp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(parsed["id"].is_null());
    }

    #[test]
    fn response_roundtrip() {
        let resp = RpcResponse::success(Some(json!("abc")), json!({"key": "value"}));
        let s = serde_json::to_string(&resp).unwrap();
        let parsed: RpcResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.id, Some(json!("abc")));
        assert_eq!(parsed.result, Some(json!({"key": "value"})));
        assert!(parsed.error.is_none());
    }
}
