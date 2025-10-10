mod copy_file;
mod local;
mod patch_file;
mod ssh;

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
