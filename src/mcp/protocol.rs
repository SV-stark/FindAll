//! JSON-RPC 2.0 / MCP envelope types and helpers.

use serde_json::{Value, json};

/// Error codes used by this server.
///
/// These are the standard JSON-RPC codes plus the MCP-relevant `-32602`
/// (invalid params), which is what a client sees when a tool argument fails
/// validation.
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

/// An RPC-level error.
#[derive(Debug)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl RpcError {
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS, message)
    }
}

/// Wraps a result value in a JSON-RPC success envelope.
///
/// `result` is taken by reference because `json!` borrows it when building the
/// envelope, so there is no reason to force callers to give up ownership.
#[must_use]
pub fn success_response(id: Option<Value>, result: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "result": result,
    })
}

/// Wraps a JSON-RPC error envelope.
#[must_use]
pub fn error_object(id: Option<Value>, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": { "code": code, "message": message },
    })
}

/// Builds a bare HTTP error response body carrying a JSON-RPC error.
///
/// Used for failures detected before (or instead of) a valid request envelope
/// exists, e.g. an oversized body or a JSON parse error.
#[must_use]
pub fn error_response(id: Option<Value>, code: i32, message: &str) -> axum::response::Response {
    use axum::response::IntoResponse;
    axum::Json(error_object(id, code, message)).into_response()
}

/// The `initialize` result.
#[must_use]
pub fn initialize_result() -> Value {
    json!({
        "protocolVersion": crate::mcp::PROTOCOL_VERSION,
        "capabilities": {
            // Read-only: no tools that mutate state, no resources, no prompts.
            "tools": { "listChanged": false }
        },
        "serverInfo": {
            "name": "flash-search",
            "version": env!("CARGO_PKG_VERSION"),
        }
    })
}

/// The `tools/list` result.
#[must_use]
pub fn tools_list_result() -> Value {
    json!({ "tools": crate::mcp::tools::tool_definitions() })
}

/// Builds a `tools/call` result.
///
/// MCP distinguishes a *tool* error (the tool ran and failed, surfaced in the
/// payload with `isError: true`) from a *protocol* error (bad arguments). Tool
/// failures are reported this way so the model can see and react to them,
/// rather than the client seeing an opaque transport error.
#[must_use]
pub fn tool_result_with_error(message: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": message.into() }],
        "isError": true,
    })
}

/// Builds a successful `tools/call` result from pre-rendered text content.
#[must_use]
pub fn tool_result_text(text: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": text.into() }],
        "isError": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_envelope_carries_the_id() {
        let response = success_response(Some(json!(42)), &json!({"ok": true}));
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 42);
        assert_eq!(response["result"]["ok"], true);
        assert!(response.get("error").is_none());
    }

    #[test]
    fn error_envelope_carries_code_and_message() {
        let response = error_object(Some(json!("abc")), METHOD_NOT_FOUND, "nope");
        assert_eq!(response["id"], "abc");
        assert_eq!(response["error"]["code"], -32601);
        assert_eq!(response["error"]["message"], "nope");
        assert!(response.get("result").is_none());
    }

    #[test]
    fn initialize_advertises_tools_and_no_mutating_capabilities() {
        let result = initialize_result();
        assert_eq!(result["protocolVersion"], crate::mcp::PROTOCOL_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
        // A read-only server must not claim resources/prompts/logging support.
        assert!(result["capabilities"].get("resources").is_none());
        assert!(result["capabilities"].get("prompts").is_none());
    }

    #[test]
    fn tool_error_is_flagged_for_the_model() {
        let result = tool_result_with_error("file not found");
        assert_eq!(result["isError"], true);
        assert_eq!(result["content"][0]["text"], "file not found");
    }
}
