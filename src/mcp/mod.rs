//! MCP (Model Context Protocol) server over HTTP.
//!
//! Replaces the previous bespoke raw-TCP protocol on `127.0.0.1:9095`, which
//! spoke a one-line-token/one-line-query format that no agent could speak. This
//! is a spec-compliant JSON-RPC 2.0 endpoint implementing the MCP `2025-03-26`
//! transport, exposing the search index as read-only tools.
//!
//! `AnyTXT` ships the same shape (HTTP API + MCP, same protocol revision,
//! read-only tools). The difference is depth rather than presence: these tools
//! expose the full operator language, the filename index, and on-disk
//! extraction.
//!
//! # Security
//!
//! - Loopback-only bind. Never `0.0.0.0`: the index reveals every indexed path
//!   and the text inside them.
//! - Bearer-token auth using the existing per-user `ipc_token`.
//! - Request bodies are size-capped before being buffered.
//! - Every tool is read-only. Nothing here mutates the index, deletes files, or
//!   touches the filesystem outside of reading a file the user already indexed.

pub mod protocol;
pub mod tools;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::Value;
use std::sync::Arc;

use crate::commands::AppState;

/// MCP protocol revision implemented here.
pub const PROTOCOL_VERSION: &str = "2025-03-26";

/// Loopback address the server binds. Deliberately not configurable to a
/// non-loopback address without an explicit, warned-about opt-in.
pub const BIND_ADDR: &str = "127.0.0.1:9095";

/// Maximum accepted request body size.
///
/// Tool arguments are small (a query string plus numeric bounds). Anything
/// larger is either a mistake or an attempt to exhaust memory.
const MAX_BODY_BYTES: usize = 64 * 1024;

/// Shared server state.
pub struct McpServer {
    pub state: Arc<AppState>,
    pub token: String,
}

/// Starts the MCP server on [`BIND_ADDR`].
///
/// A bind failure is logged and returns, never fatal: search must keep working
/// if the agent-integration surface cannot start.
pub async fn serve(state: Arc<AppState>, token: String) {
    let app_state = McpServer { state, token };

    let router = Router::new()
        .route("/mcp", post(handle_rpc))
        .with_state(Arc::new(app_state));

    let listener = match tokio::net::TcpListener::bind(BIND_ADDR).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind MCP server at {BIND_ADDR}: {e}");
            return;
        }
    };

    tracing::info!("MCP server listening on http://{BIND_ADDR}/mcp (bearer token required)");

    if let Err(e) = axum::serve(listener, router).await {
        tracing::error!("MCP server stopped: {e}");
    }
}

/// Single JSON-RPC entry point.
async fn handle_rpc(
    State(server): State<Arc<McpServer>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if let Err(response) = authorize(&server, &headers) {
        return *response;
    }

    // Cap before parsing: an oversized body must not be deserialized just to be
    // rejected afterwards.
    if body.len() > MAX_BODY_BYTES {
        return protocol::error_response(
            None,
            -32600,
            &format!("request body exceeds {MAX_BODY_BYTES} bytes"),
        );
    }

    let request: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return protocol::error_response(None, -32700, &format!("parse error: {e}"));
        }
    };

    // Batches are optional in JSON-RPC; a batch gets one response array.
    if let Some(items) = request.as_array() {
        if items.is_empty() {
            return protocol::error_response(None, -32600, "empty batch");
        }
        let mut responses = Vec::with_capacity(items.len());
        for item in items {
            if let Some(response) = dispatch(&server, item).await {
                responses.push(response);
            }
        }
        if responses.is_empty() {
            return StatusCode::ACCEPTED.into_response();
        }
        return Json(responses).into_response();
    }

    // A notification has no id and expects no response body.
    dispatch(&server, &request).await.map_or_else(
        || StatusCode::ACCEPTED.into_response(),
        |response| Json(response).into_response(),
    )
}

/// Validates the bearer token.
///
/// An absent or wrong token is a hard 401 with no detail, so the endpoint
/// cannot be probed for a valid token.
/// Boxed because an `axum::response::Response` is large; returning it inline
/// would inflate every caller's stack frame for no benefit.
type AuthResult = Result<(), Box<Response>>;

fn authorize(server: &McpServer, headers: &HeaderMap) -> AuthResult {
    let presented = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim);

    match presented {
        Some(token)
            if !server.token.is_empty()
                && constant_time_eq(token.as_bytes(), server.token.as_bytes()) =>
        {
            Ok(())
        }
        _ => {
            tracing::warn!("Rejected unauthenticated MCP request");
            Err(Box::new(
                (
                    StatusCode::UNAUTHORIZED,
                    [("www-authenticate", "Bearer")],
                    "unauthorized",
                )
                    .into_response(),
            ))
        }
    }
}

/// Length-independent comparison, so a caller cannot learn the token a byte at
/// a time from response timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Routes one JSON-RPC request to its handler.
///
/// Returns `None` for notifications (requests without an `id`), which per
/// JSON-RPC must not be answered.
async fn dispatch(server: &McpServer, request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str);
    let params = request.get("params").cloned().unwrap_or(Value::Null);

    let Some(method) = method else {
        return Some(protocol::error_object(id, -32600, "missing \"method\""));
    };

    let result = match method {
        "initialize" => Ok(protocol::initialize_result()),
        "ping" => Ok(Value::Object(serde_json::Map::new())),
        "tools/list" => Ok(protocol::tools_list_result()),
        "tools/call" => tools::call_tool(&server.state, &params).await,
        other => Err(protocol::RpcError {
            code: -32601,
            message: format!("method not found: {other}"),
        }),
    };

    // Notifications never get a response, success or failure.
    let is_notification = id.is_none();
    match result {
        Ok(value) => {
            if is_notification {
                None
            } else {
                Some(protocol::success_response(id, &value))
            }
        }
        Err(error) => {
            if is_notification {
                tracing::debug!("Notification {method} failed: {}", error.message);
                None
            } else {
                Some(protocol::error_object(id, error.code, &error.message))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a server whose state is only needed for the auth path.
    ///
    /// Auth is checked before any state is touched, so these tests only need a
    /// well-formed `AppState`. It runs on a Tokio runtime because the watcher
    /// captures `Handle::current()`.
    fn server(token: &str) -> McpServer {
        McpServer {
            state: tools::test_state(),
            token: token.to_string(),
        }
    }

    fn auth_header(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {token}").parse().expect("valid header"),
        );
        headers
    }

    #[tokio::test]
    async fn rejects_missing_token() {
        let server = server("secret");
        assert!(authorize(&server, &HeaderMap::new()).is_err());
    }

    #[tokio::test]
    async fn rejects_wrong_token() {
        let server = server("secret");
        assert!(authorize(&server, &auth_header("wrong")).is_err());
    }

    #[tokio::test]
    async fn accepts_correct_token() {
        let server = server("secret");
        assert!(authorize(&server, &auth_header("secret")).is_ok());
    }

    #[tokio::test]
    async fn empty_server_token_never_authenticates() {
        // A missing token file must fail closed, not open.
        let server = server("");
        assert!(authorize(&server, &auth_header("")).is_err());
        assert!(authorize(&server, &auth_header("anything")).is_err());
    }

    #[test]
    fn token_comparison_is_length_safe() {
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }

    #[tokio::test]
    async fn initialize_reports_protocol_version_and_server_info() {
        let server = server("t");
        let response = dispatch(
            &server,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": { "protocolVersion": PROTOCOL_VERSION }
            }),
        )
        .await
        .expect("response expected");

        assert_eq!(response["id"], 1);
        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert!(response["result"]["serverInfo"]["name"].is_string());
        assert!(response["result"]["capabilities"]["tools"].is_object());
    }

    #[tokio::test]
    async fn unknown_method_is_method_not_found() {
        let server = server("t");
        let response = dispatch(
            &server,
            &serde_json::json!({"jsonrpc": "2.0", "id": 7, "method": "nope"}),
        )
        .await
        .expect("response expected");
        assert_eq!(response["id"], 7);
        assert_eq!(response["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn notification_gets_no_response() {
        let server = server("t");
        // `notifications/initialized` carries no id.
        let response = dispatch(
            &server,
            &serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        )
        .await;
        assert!(response.is_none(), "notification must not be answered");
    }

    #[tokio::test]
    async fn tools_list_exposes_every_advertised_tool() {
        let server = server("t");
        let response = dispatch(
            &server,
            &serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        )
        .await
        .expect("response expected");

        let listed = response["result"]["tools"].as_array().expect("tools array");
        let names: Vec<&str> = listed.iter().filter_map(|t| t["name"].as_str()).collect();

        for expected in tools::TOOL_NAMES {
            assert!(
                names.contains(expected),
                "{expected} advertised by TOOL_NAMES but missing from tools/list: {names:?}"
            );
        }
        // Every tool must declare an input schema, or strict clients reject it.
        for tool in listed {
            assert!(
                tool["inputSchema"].is_object(),
                "tool {} has no inputSchema",
                tool["name"]
            );
        }
    }
}
