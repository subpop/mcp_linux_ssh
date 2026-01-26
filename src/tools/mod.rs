mod copy_file;
mod local;
mod patch_file;
mod ssh;

use anyhow::Error;
use expand_tilde::expand_tilde;
use std::ops::Deref;

use copy_file::CopyFile;
use local::RunLocalCommand;
use patch_file::PatchFile;
use rust_mcp_sdk::tool_box;
use ssh::{RunSSHCommand, RunSSHSudoCommand};

tool_box!(
    POSIXSSHTools,
    [
        RunLocalCommand,
        RunSSHCommand,
        RunSSHSudoCommand,
        CopyFile,
        PatchFile
    ]
);

/// Get SSH multiplexing options for a given host.
/// Returns a vector of SSH option strings that can be passed via `-o` flags.
///
/// The options enable SSH connection multiplexing:
/// - ControlMaster auto: Automatically use existing master connection or create one
/// - ControlPath: Socket path template for the control socket
/// - ControlPersist: Keep master connection alive for 10 minutes after last use
pub(crate) fn get_multiplexing_options() -> Result<Vec<String>, Error> {
    // Expand ~ to home directory for ControlPath
    let control_path_template = "~/.ssh/control-%h-%p-%r";
    let expanded_path = expand_tilde(control_path_template)
        .map_err(|e| Error::msg(format!("Failed to expand ControlPath: {}", e)))?;
    let control_path = expanded_path.deref().as_os_str().to_str().ok_or_else(|| {
        Error::msg(format!(
            "Failed to convert ControlPath to string: {}",
            control_path_template
        ))
    })?;

    Ok(vec![
        "ControlMaster=auto".to_string(),
        format!("ControlPath={}", control_path),
        "ControlPersist=10m".to_string(),
    ])
}

fn map_from_output(
    stdout: String,
    stderr: String,
    status_code: Option<i32>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut structured_content = serde_json::Map::new();
    structured_content.insert("stdout".to_string(), serde_json::Value::String(stdout));
    structured_content.insert("stderr".to_string(), serde_json::Value::String(stderr));
    structured_content.insert(
        "status_code".to_string(),
        match status_code {
            Some(code) => serde_json::Value::Number(code.into()),
            None => serde_json::Value::Null,
        },
    );
    structured_content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_multiplexing_options() {
        let options = get_multiplexing_options().unwrap();
        assert!(options.len() >= 3);
        assert!(options.iter().any(|opt| opt.starts_with("ControlMaster=")));
        assert!(options.iter().any(|opt| opt.starts_with("ControlPath=")));
        assert!(options.iter().any(|opt| opt.starts_with("ControlPersist=")));
    }
}
