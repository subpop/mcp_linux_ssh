use async_trait::async_trait;
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use rust_mcp_sdk::schema::{
    CallToolRequest, CallToolResult, ListToolsRequest, ListToolsResult, RpcError,
};
use rust_mcp_sdk::{McpServer, mcp_server::ServerHandler};
use std::sync::Arc;

use crate::tools::POSIXSSHTools;

pub struct POSIXSSHHandler;

#[async_trait]
impl ServerHandler for POSIXSSHHandler {
    /// Handle list tool requests
    async fn handle_list_tools_request(
        &self,
        _: ListToolsRequest,
        _: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            meta: None,
            next_cursor: None,
            tools: POSIXSSHTools::tools(),
        })
    }

    /// Handle tool call requests
    async fn handle_call_tool_request(
        &self,
        request: CallToolRequest,
        _: Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        let params = POSIXSSHTools::try_from(request.params).map_err(CallToolError::new)?;

        match params {
            POSIXSSHTools::RunLocalCommand(tool) => tool.call_tool().await,
            POSIXSSHTools::RunSSHCommand(tool) => tool.call_tool().await,
            POSIXSSHTools::RunSSHSudoCommand(tool) => tool.call_tool().await,
            POSIXSSHTools::CopyFile(tool) => tool.call_tool().await,
            POSIXSSHTools::PatchFile(tool) => tool.call_tool().await,
        }
    }
}
