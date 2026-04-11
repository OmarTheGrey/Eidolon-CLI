//! MCP server mode — expose Eidolon as a stdio MCP server.
//!
//! Other MCP clients (Claude Desktop, Cursor, VS Code) can connect to Eidolon
//! and browse session history or search across conversations via standard MCP
//! protocol tools.

use std::io::{self, BufRead, Write};
use std::path::Path;

use serde_json::{json, Value};

use crate::mcp_stdio::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::session_index::SessionIndex;

const SERVER_NAME: &str = "eidolon";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Run the MCP server on stdin/stdout. Blocks until stdin is closed.
/// The `sessions_dir` is used to open/create the session search index.
pub fn run_mcp_server(sessions_dir: &Path) -> Result<(), String> {
    let index_path = sessions_dir.join("session_index.db");
    let index = SessionIndex::open(&index_path)?;

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| format!("stdin read error: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let error_response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: JsonRpcId::Null,
                    result: None,
                    error: Some(crate::mcp_stdio::JsonRpcError {
                        code: -32700,
                        message: format!("parse error: {e}"),
                        data: None,
                    }),
                };
                write_response(&mut out, &error_response)?;
                continue;
            }
        };

        let response = handle_request(&request, &index);
        write_response(&mut out, &response)?;
    }

    Ok(())
}

fn write_response(out: &mut impl Write, response: &JsonRpcResponse) -> Result<(), String> {
    let json = serde_json::to_string(response).map_err(|e| format!("serialize error: {e}"))?;
    writeln!(out, "{json}").map_err(|e| format!("stdout write error: {e}"))?;
    out.flush().map_err(|e| format!("stdout flush error: {e}"))?;
    Ok(())
}

fn handle_request(request: &JsonRpcRequest, index: &SessionIndex) -> JsonRpcResponse {
    let result: Result<Value, (i64, String)> = match request.method.as_str() {
        "initialize" => Ok(handle_initialize()),
        "tools/list" => Ok(handle_list_tools()),
        "tools/call" => {
            let params = request.params.clone().unwrap_or(json!({}));
            handle_tool_call(&params, index)
        }
        "notifications/initialized" | "ping" => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: Some(json!({})),
                error: None,
            };
        }
        _ => Err((-32601, format!("method not found: {}", request.method))),
    };

    match result {
        Ok(value) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: Some(value),
            error: None,
        },
        Err((code, message)) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: None,
            error: Some(crate::mcp_stdio::JsonRpcError {
                code,
                message,
                data: None,
            }),
        },
    }
}

fn handle_initialize() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": SERVER_VERSION
        }
    })
}

fn handle_list_tools() -> Value {
    json!({
        "tools": [
            {
                "name": "session_search",
                "description": "Search across all past Eidolon conversations by keyword. Returns ranked results with session IDs, roles, content snippets, and timestamps.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Full-text search query (FTS5 syntax: plain keywords, quoted phrases, or boolean operators)"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of results to return",
                            "default": 10
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "session_stats",
                "description": "Get statistics about the session index (number of indexed messages).",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }
        ]
    })
}

fn handle_tool_call(params: &Value, index: &SessionIndex) -> Result<Value, (i64, String)> {
    let tool_name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing tool name".to_string()))?;
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    match tool_name {
        "session_search" => {
            let query = arguments
                .get("query")
                .and_then(Value::as_str)
                .ok_or((-32602, "missing required field: query".to_string()))?;
            let limit = arguments
                .get("limit")
                .and_then(Value::as_u64)
                .map_or(10, |v| usize::try_from(v).unwrap_or(10));

            let results = index
                .search(query, limit)
                .map_err(|e| (-32603, format!("search failed: {e}")))?;

            let items: Vec<Value> = results
                .iter()
                .map(|r| {
                    json!({
                        "session_id": r.session_id,
                        "role": r.role,
                        "content": truncate(&r.content, 500),
                        "timestamp": r.timestamp,
                        "rank": r.rank,
                    })
                })
                .collect();

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&items).unwrap_or_default()
                }]
            }))
        }
        "session_stats" => Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Indexed messages: {}", index.message_count())
            }]
        })),
        _ => Err((-32602, format!("unknown tool: {tool_name}"))),
    }
}

fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..s.floor_char_boundary(max_len)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_index() -> SessionIndex {
        let db_path = std::env::temp_dir().join(format!(
            "eidolon-mcp-server-test-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let index = SessionIndex::open(&db_path).unwrap();
        index
            .index_message("s1", "user", "how does permission enforcement work", 1000)
            .unwrap();
        index
            .index_message("s1", "assistant", "the permission system uses PermissionPolicy", 1001)
            .unwrap();
        index
    }

    #[test]
    fn initialize_returns_server_info() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1),
            method: "initialize".to_string(),
            params: Some(json!({})),
        };
        let index = test_index();
        let response = handle_request(&request, &index);
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert_eq!(result["serverInfo"]["name"], "eidolon");
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
    }

    #[test]
    fn list_tools_returns_session_search() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(2),
            method: "tools/list".to_string(),
            params: Some(json!({})),
        };
        let index = test_index();
        let response = handle_request(&request, &index);
        assert!(response.error.is_none());
        let tools = response.result.unwrap()["tools"].as_array().unwrap().clone();
        assert!(tools.iter().any(|t| t["name"] == "session_search"));
        assert!(tools.iter().any(|t| t["name"] == "session_stats"));
    }

    #[test]
    fn tool_call_session_search_returns_results() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(3),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "session_search",
                "arguments": { "query": "permission" }
            })),
        };
        let index = test_index();
        let response = handle_request(&request, &index);
        assert!(response.error.is_none());
        let content = &response.result.unwrap()["content"][0]["text"];
        let text = content.as_str().unwrap();
        assert!(text.contains("permission"));
    }

    #[test]
    fn unknown_method_returns_error() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(4),
            method: "nonexistent/method".to_string(),
            params: Some(json!({})),
        };
        let index = test_index();
        let response = handle_request(&request, &index);
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32601);
    }
}
