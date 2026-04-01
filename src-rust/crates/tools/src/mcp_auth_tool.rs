// McpAuthTool: pseudo-tool surfaced for MCP servers that require OAuth.
//
// Tool name: "mcp__{server_name}__authenticate"
//
// In the TypeScript implementation this is a dynamically-created tool that
// triggers the full OAuth flow and returns the authorisation URL.  The Rust
// port provides a static McpAuthTool that inspects the ToolContext.mcp_manager
// and the supplied server_name to return the server's connection status.
//
// Full OAuth is not yet implemented in cc-mcp; this tool returns the server
// URL (if any) with instructions for manual browser auth, matching the
// "unsupported transport" branch in the TypeScript code for non-HTTP servers.

use crate::{PermissionLevel, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct McpAuthTool;

#[derive(Debug, Deserialize)]
struct McpAuthInput {
    server_name: String,
}

#[async_trait]
impl Tool for McpAuthTool {
    fn name(&self) -> &str {
        "mcp__auth"
    }

    fn description(&self) -> &str {
        "Start the OAuth authorisation flow for an MCP server that requires \
         authentication. Returns the authorisation URL the user should open \
         in their browser. If the server does not support OAuth from this tool, \
         instructions for manual authentication are returned instead."
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::None
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "server_name": {
                    "type": "string",
                    "description": "The MCP server name that needs authentication."
                }
            },
            "required": ["server_name"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let params: McpAuthInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid input: {}", e)),
        };

        let manager = match &ctx.mcp_manager {
            Some(m) => m,
            None => {
                return ToolResult::error(
                    "No MCP manager configured. Cannot authenticate MCP servers.".to_string(),
                )
            }
        };

        // Check server status
        let status = manager.server_status(&params.server_name);

        use cc_mcp::McpServerStatus;
        match status {
            McpServerStatus::Connected { tool_count } => ToolResult::success(format!(
                "MCP server \"{}\" is already connected ({} tool(s) available). \
                 No authentication needed.",
                params.server_name, tool_count
            )),
            McpServerStatus::Connecting => ToolResult::success(format!(
                "MCP server \"{}\" is currently connecting. Try again in a moment.",
                params.server_name
            )),
            McpServerStatus::Failed { error, .. } => ToolResult::error(format!(
                "MCP server \"{}\" failed to connect: {}. \
                 Run /mcp in the Claude interface to retry or authenticate manually.",
                params.server_name, error
            )),
            McpServerStatus::Disconnected { last_error } => {
                // Full OAuth is not yet implemented in cc-mcp.
                // Return instructions matching the TS "unsupported transport" fallback.
                let err_note = last_error
                    .map(|e| format!(" Last error: {}", e))
                    .unwrap_or_default();

                ToolResult::success(
                    serde_json::json!({
                        "status": "unsupported",
                        "message": format!(
                            "MCP server \"{}\" is disconnected.{} \
                             OAuth from this tool is not yet supported in the Rust runtime. \
                             Run /mcp in the Claude interface and authenticate \"{}\" manually.",
                            params.server_name, err_note, params.server_name
                        )
                    })
                    .to_string(),
                )
            }
        }
    }
}
