use anyhow::Error;
use rust_mcp_sdk::{
    macros::{JsonSchema, mcp_tool},
    schema::{CallToolResult, TextContent, schema_utils::CallToolError},
};
use tokio::{
    process::Command,
    time::{Duration, timeout},
};

#[mcp_tool(
    name = "run_ssh_command",
    description = "Run a command on a remote POSIX compatible system (Linux, BSD, macOS) system and return the output. This tool does not permit commands to be run with sudo.",
    title = "Run SSH Command"
)]
#[derive(Debug, ::serde::Serialize, ::serde::Deserialize, JsonSchema)]
pub struct RunSSHCommand {
    /// The host to run the command on. Can be a host alias from ~/.ssh/config, a hostname, or an IP address.
    pub remote_host: String,
    /// The command to run. This must be a single command. Arguments must be passed in the args parameter.
    pub cmd: String,
    /// The arguments to pass to the command.
    pub args: Vec<String>,
    /// Timeout in seconds for the command execution. Defaults to 30 seconds. Set to 0 to disable timeout.
    pub timeout_seconds: Option<u64>,
    /// Additional options to pass to the ssh command. Each option should be a key-value pair separated by an equal sign (=). The options are passed to the ssh command using the -o flag.
    pub options: Option<Vec<String>>,
}

impl RunSSHCommand {
    #[tracing::instrument(skip(self))]
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let _span = tracing::span!(tracing::Level::TRACE, "run_ssh_command", cmd = ?self.cmd, args = ?self.args, timeout_seconds = ?self.timeout_seconds);
        let _enter = _span.enter();

        let timeout_seconds = self.timeout_seconds.unwrap_or(30);
        let options_vec: Option<Vec<&str>> = self
            .options
            .as_ref()
            .map(|v| v.iter().map(String::as_str).collect());

        if self.cmd.contains("sudo") || self.args.iter().any(|arg| arg.contains("sudo")) {
            // sudo is not permitted for this tool.
            return Err(CallToolError::from_message(
                "You may not run commands with sudo using this tool",
            ));
        }

        match exec_ssh(
            &self.remote_host,
            &self.cmd,
            &self
                .args
                .iter()
                .map(|arg| arg.as_str())
                .collect::<Vec<&str>>(),
            timeout_seconds,
            options_vec.as_deref(),
        )
        .await
        {
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
                "Failed to execute remote SSH command: {}",
                err
            ))),
        }
    }
}

#[mcp_tool(
    name = "run_ssh_sudo_command",
    description = "Run a command on a remote POSIX compatible system (Linux, \
    BSD, macOS) system and return the output. This tool explicitly runs \
    commands with sudo.",
    title = "Run SSH Sudo Command"
)]
#[derive(Debug, ::serde::Serialize, ::serde::Deserialize, JsonSchema)]
pub struct RunSSHSudoCommand {
    /// The host to run the command on. Can be a host alias from ~/.ssh/config, a hostname, or an IP address.
    pub remote_host: String,
    /// The command to run. This must be a single command. Arguments must be passed in the args parameter.
    pub cmd: String,
    /// The arguments to pass to the command.
    pub args: Vec<String>,
    /// Timeout in seconds for the command execution. Defaults to 30 seconds. Set to 0 to disable timeout.
    pub timeout_seconds: Option<u64>,
    /// Additional options to pass to the ssh command. Each option should be a key-value pair separated by an equal sign (=). The options are passed to the ssh command using the -o flag.
    pub options: Option<Vec<String>>,
}

impl RunSSHSudoCommand {
    #[tracing::instrument(skip(self))]
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let _span = tracing::span!(tracing::Level::TRACE, "run_ssh_sudo_command", cmd = ?self.cmd, args = ?self.args, timeout_seconds = ?self.timeout_seconds);
        let _enter = _span.enter();

        let timeout_seconds = self.timeout_seconds.unwrap_or(30);
        let options_vec: Option<Vec<&str>> = self
            .options
            .as_ref()
            .map(|v| v.iter().map(String::as_str).collect());

        match exec_ssh(
            &self.remote_host,
            "sudo",
            std::iter::once(self.cmd.as_str())
                .chain(self.args.iter().map(|arg| arg.as_str()))
                .collect::<Vec<&str>>()
                .as_slice(),
            timeout_seconds,
            options_vec.as_deref(),
        )
        .await
        {
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
                "Failed to execute remote SSH command with sudo: {}",
                err
            ))),
        }
    }
}

/// Run a command on a remote POSIX compatible system (Linux, BSD, macOS) system
/// via SSH.
#[tracing::instrument]
async fn exec_ssh(
    host: &str,
    command: &str,
    args: &[&str],
    timeout_seconds: u64,
    options: Option<&[&str]>,
) -> Result<std::process::Output, Error> {
    let _span = tracing::span!(tracing::Level::TRACE, "exec_ssh", host = %host, command = %command, args = ?args, timeout_seconds = %timeout_seconds);
    let _enter = _span.enter();

    // Get multiplexing options
    let multiplexing_opts = super::get_multiplexing_options()
        .map_err(|e| Error::msg(format!("Failed to get multiplexing options: {}", e)))?;

    // Build SSH command with multiplexing enabled
    let mut cmd = Command::new("ssh");
    cmd.arg(host);

    // Always append StrictHostKeyChecking=yes to ensure SSH fails instead of prompting interactively
    cmd.args(["-o", "StrictHostKeyChecking=yes"]);

    // Add multiplexing options first to ensure they take precedence
    for opt in &multiplexing_opts {
        cmd.args(["-o", opt]);
    }

    // Add user-provided options last
    if let Some(opts) = options {
        for opt in opts {
            cmd.args(["-o", opt]);
        }
    }

    // Add command and arguments
    cmd.arg(command).args(args);

    let command_future = cmd.output();

    let result = if timeout_seconds == 0 {
        // No timeout - run indefinitely
        command_future.await
    } else {
        // Apply timeout
        let timeout_duration = Duration::from_secs(timeout_seconds);
        match timeout(timeout_duration, command_future).await {
            Ok(result) => result,
            Err(_) => {
                return Err(Error::msg(format!(
                    "SSH command timed out after {} seconds",
                    timeout_seconds
                )));
            }
        }
    };

    result.map_err(|e| Error::msg(format!("Failed to run SSH command: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_ssh_command_rejects_sudo() {
        let cmd = RunSSHCommand {
            remote_host: "localhost".to_string(),
            cmd: "sudo".to_string(),
            args: vec!["ls".to_string()],
            timeout_seconds: Some(1),
            options: None,
        };

        let result = cmd.call_tool().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("sudo"));
    }

    #[test]
    fn test_run_ssh_sudo_command_struct_creation() {
        let cmd = RunSSHSudoCommand {
            remote_host: "localhost".to_string(),
            cmd: "apt".to_string(),
            args: vec!["update".to_string()],
            timeout_seconds: Some(60),
            options: None,
        };

        assert_eq!(cmd.remote_host, "localhost");
        assert_eq!(cmd.cmd, "apt");
    }
}
