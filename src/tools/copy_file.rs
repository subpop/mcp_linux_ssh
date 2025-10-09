use expand_tilde::expand_tilde;
use rust_mcp_sdk::{
    macros::{JsonSchema, mcp_tool},
    schema::{CallToolResult, TextContent, schema_utils::CallToolError},
};
use std::ops::Deref;
use tokio::{
    process::Command,
    time::{Duration, timeout},
};

#[mcp_tool(
    name = "copy_file",
    description = "Copy a file from the local machine to a remote POSIX compatible system (Linux, BSD, macOS) using rsync over SSH. Preserves file attributes and creates a backup if the destination file already exists.",
    title = "Copy File"
)]
#[derive(Debug, ::serde::Serialize, ::serde::Deserialize, JsonSchema)]
pub struct CopyFile {
    /// The source file path on the local machine.
    pub source: String,
    /// The destination file path on the remote machine.
    pub destination: String,
    /// The user to run the command as. Defaults to the current username.
    pub remote_user: Option<String>,
    /// The host to copy the file to.
    pub remote_host: String,
    /// The private key to use for authentication. Defaults to ~/.ssh/id_ed25519.
    pub private_key: Option<String>,
    /// Timeout in seconds for the command execution. Defaults to 30 seconds. Set to 0 to disable timeout.
    pub timeout_seconds: Option<u64>,
}

impl CopyFile {
    #[tracing::instrument(skip(self))]
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let _span = tracing::span!(tracing::Level::TRACE, "copy_file", source = ?self.source, destination = ?self.destination);
        let _enter = _span.enter();

        let source = expand_tilde(&self.source).map_err(|e| {
            CallToolError::from_message(format!("Failed to expand source path: {}", e))
        })?;

        let remote_user = self.remote_user.clone().unwrap_or(whoami::username());
        let private_key = self
            .private_key
            .clone()
            .unwrap_or("~/.ssh/id_ed25519".to_string());
        let timeout_seconds = self.timeout_seconds.unwrap_or(30);

        // Expand the private key path
        let expanded_key = expand_tilde(&private_key).map_err(|e| {
            CallToolError::from_message(format!("Failed to expand private key path: {}", e))
        })?;
        let private_key_path = expanded_key.deref().as_os_str().to_str().ok_or_else(|| {
            CallToolError::from_message(format!(
                "Failed to convert private key to string: {}",
                private_key
            ))
        })?;

        let ssh_command = format!("ssh -i {}", private_key_path);
        let remote_target = format!("{}@{}:{}", remote_user, self.remote_host, self.destination);

        // Build the rsync command
        // -a: archive mode (preserves permissions, timestamps, etc.)
        // -v: verbose
        // -b: create backups of existing files
        // -e: specify ssh command with identity file
        let command_future = Command::new("rsync")
            .arg("-avb")
            .arg("-e")
            .arg(&ssh_command)
            .arg(source.to_string_lossy().into_owned())
            .arg(&remote_target)
            .output();

        let result = if timeout_seconds == 0 {
            // No timeout - run indefinitely
            command_future.await
        } else {
            // Apply timeout
            let timeout_duration = Duration::from_secs(timeout_seconds);
            match timeout(timeout_duration, command_future).await {
                Ok(result) => result,
                Err(_) => {
                    return Err(CallToolError::from_message(format!(
                        "rsync command timed out after {} seconds",
                        timeout_seconds
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
            Err(e) => Err(CallToolError::from_message(format!(
                "Failed to execute rsync command: {}",
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_file_struct_creation() {
        let copy = CopyFile {
            source: "/tmp/test.txt".to_string(),
            destination: "/home/user/test.txt".to_string(),
            remote_user: Some("testuser".to_string()),
            remote_host: "localhost".to_string(),
            private_key: Some("~/.ssh/test_key".to_string()),
            timeout_seconds: Some(60),
        };

        assert_eq!(copy.source, "/tmp/test.txt");
        assert_eq!(copy.destination, "/home/user/test.txt");
        assert_eq!(copy.remote_host, "localhost");
    }

    #[test]
    fn test_copy_file_defaults() {
        let copy = CopyFile {
            source: "file.txt".to_string(),
            destination: "/remote/path/file.txt".to_string(),
            remote_user: None,
            remote_host: "example.com".to_string(),
            private_key: None,
            timeout_seconds: None,
        };

        assert!(copy.remote_user.is_none());
        assert!(copy.private_key.is_none());
        assert!(copy.timeout_seconds.is_none());
    }
}
