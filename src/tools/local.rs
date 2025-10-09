use rust_mcp_sdk::{
    macros::{JsonSchema, mcp_tool},
    schema::{CallToolResult, TextContent, schema_utils::CallToolError},
};
use tokio::{
    process::Command,
    time::{Duration, timeout},
};

#[mcp_tool(
    name = "run_local_command",
    description = "Run a command on the local system and return the output. Use this sparingly; only when needed to troubleshoot why connecting to the remote system is failing.",
    title = "Run a local command"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct RunLocalCommand {
    /// The command to run. This must be a single command. Arguments must be passed in the args parameter.
    cmd: String,
    /// The arguments to pass to the command.
    args: Vec<String>,
    /// Timeout in seconds for the command execution. Defaults to 30 seconds. Set to 0 to disable timeout.
    timeout_seconds: Option<u64>,
}

impl RunLocalCommand {
    #[tracing::instrument(skip(self))]
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let _span = tracing::span!(tracing::Level::TRACE, "run_local_command", cmd = ?self.cmd, args = ?self.args, timeout_seconds = ?self.timeout_seconds);
        let _enter = _span.enter();

        let command_future = Command::new(&self.cmd).args(&self.args).output();

        let result = if self.timeout_seconds == Some(0) {
            // No timeout - run indefinitely
            command_future.await
        } else {
            // Apply timeout
            let timeout_duration = Duration::from_secs(self.timeout_seconds.unwrap_or(30));
            match timeout(timeout_duration, command_future).await {
                Ok(result) => result,
                Err(_) => {
                    return Err(CallToolError::from_message(format!(
                        "Local command timed out after {:?} seconds",
                        self.timeout_seconds
                    )));
                }
            }
        };

        match result {
            Ok(output) => {
                // The command executed successfully. This doesn't mean it
                // succeeded, so output is returned as a successful tool call.
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let status_code = output.status.code();

                Ok(
                    CallToolResult::text_content(vec![TextContent::from(stdout.clone())])
                        .with_structured_content(super::map_from_output(
                            stdout,
                            stderr,
                            status_code,
                        )),
                )
            }
            Err(err) => Err(CallToolError::from_message(format!(
                "Failed to run local command: {}",
                err
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_local_command_success() {
        let cmd = RunLocalCommand {
            cmd: "echo".to_string(),
            args: vec!["hello".to_string()],
            timeout_seconds: None,
        };

        let result = cmd.call_tool().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_local_command_nonexistent() {
        let cmd = RunLocalCommand {
            cmd: "nonexistent_command_12345".to_string(),
            args: vec![],
            timeout_seconds: None,
        };

        let result = cmd.call_tool().await;
        assert!(result.is_err());
    }
}
