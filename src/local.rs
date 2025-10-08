use anyhow::Result;
use rmcp::{ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use tokio::process::Command;
use tokio::time::{Duration, timeout};

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct RunCommandLocalParams {
    /// The command to run. This must be a single command. Arguments must be
    /// passed in the args parameter.
    pub command: String,
    /// The arguments to pass to the command.
    pub args: Vec<String>,
    /// Timeout in seconds for the command execution. Defaults to 30 seconds.
    /// Set to 0 to disable timeout.
    pub timeout_seconds: Option<u64>,
}

impl super::Handler {
    #[tool(
        name = "Run",
        description = "Run a command on the local system and return the output. \
        Use this sparingly; only when needed to troubleshoot why connecting to the \
        remote system is failing."
    )]
    #[tracing::instrument(skip(self))]
    pub async fn run_command_local(
        &self,
        params: Parameters<RunCommandLocalParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _span = tracing::span!(tracing::Level::DEBUG, "run_command_local", params = ?params);
        let _enter = _span.enter();

        let command = params.0.command;
        let args = params.0.args;
        let timeout_seconds = params.0.timeout_seconds.unwrap_or(30);

        let command_future = Command::new(&command).args(&args).output();

        let result = if timeout_seconds == 0 {
            // No timeout - run indefinitely
            command_future.await
        } else {
            // Apply timeout
            let timeout_duration = Duration::from_secs(timeout_seconds);
            match timeout(timeout_duration, command_future).await {
                Ok(result) => result,
                Err(_) => {
                    return Err(ErrorData::internal_error(
                        format!("Local command timed out after {} seconds", timeout_seconds),
                        None,
                    ));
                }
            }
        };

        match result {
            Ok(output) => {
                // The command executed successfully. This doesn't mean it
                // succeeded, so output is returned as a successful tool call.
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let status_code = output.status.code();

                Ok(CallToolResult::structured(json!({
                    "status_code": status_code,
                    "stdout": stdout.trim().to_string(),
                    "stderr": stderr.trim().to_string(),
                })))
            }
            Err(e) => Err(ErrorData::internal_error(
                // The command failed to execute. Return the error to the caller.
                format!("Failed to execute local command: {}", e),
                None,
            )),
        }
    }
}
