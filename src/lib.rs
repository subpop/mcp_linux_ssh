use anyhow::Result;
use rmcp::{
    ErrorData, RoleServer,
    handler::server::{ServerHandler, router::prompt::PromptRouter, tool::ToolRouter},
    model::{
        AnnotateAble, GetPromptRequestParam, GetPromptResult, Implementation, ListPromptsResult,
        ListResourceTemplatesResult, ListResourcesResult, PaginatedRequestParam, RawResource,
        ReadResourceRequestParam, ReadResourceResult, ResourceContents, ServerCapabilities,
        ServerInfo,
    },
    prompt_handler, prompt_router,
    service::RequestContext,
    tool_handler, tool_router,
};
use std::fs;
use url::Url;

mod file_ops;
mod local;
mod ssh;

/// Handler for the MCP server.
#[derive(Clone, Debug, Default)]
pub struct Handler {
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

#[tool_router]
#[prompt_router]
impl Handler {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        }
    }
}

#[tool_handler]
#[prompt_handler]
impl ServerHandler for Handler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: String::from("Linux admin utilities"),
                title: Some(String::from("POSIX administration using SSH")),
                ..Default::default()
            },
            instructions: Some(String::from(
                "You are an expert POSIX compatible system (Linux, BSD, macOS) system \
                administrator. You run commands on a remote POSIX compatible system \
                (Linux, BSD, macOS) system to troubleshoot, fix issues and perform \
                general administration.",
            )),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .enable_resources()
                .build(),
            ..Default::default()
        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult {
            resources: vec![
                RawResource {
                    uri: "file:///public_keys".to_string(),
                    name: "public_keys".to_string(),
                    title: Some("public_keys".to_string()),
                    description: Some("List public keys available on the local system".to_string()),
                    mime_type: Some("text/plain".to_string()),
                    size: None,
                    icons: None,
                }
                .no_annotation(),
            ],
            next_cursor: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![],
            next_cursor: None,
        })
    }

    #[tracing::instrument(skip(self, _context))]
    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        let _span = tracing::span!(tracing::Level::TRACE, "read_resource", uri = %request.uri);
        let _enter = _span.enter();

        // Parse the URI into a URL struct
        let url = Url::parse(&request.uri)
            .map_err(|e| ErrorData::invalid_request(format!("Invalid URI: {}", e), None))?;

        match url.scheme() {
            "file" => {
                let path = url.to_file_path().map_err(|_| {
                    ErrorData::invalid_request("Invalid file URI".to_string(), None)
                })?;
                let path_str = path.to_str().ok_or_else(|| {
                    ErrorData::invalid_request("Cannot convert path to string", None)
                })?;

                match path_str {
                    "/public_keys" => {
                        // Find all public keys in ~/.ssh
                        let base_dirs = directories::BaseDirs::new().ok_or_else(|| {
                            ErrorData::internal_error("Cannot get home directory", None)
                        })?;
                        let home_dir = base_dirs.home_dir();
                        let ssh_dir = home_dir.join(".ssh");
                        let iter = fs::read_dir(&ssh_dir).map_err(|e| {
                            ErrorData::internal_error(
                                format!("Failed to read directory: {}", e),
                                None,
                            )
                        })?;

                        let mut public_keys = Vec::new();
                        for entry in iter {
                            let entry = entry.map_err(|e| {
                                ErrorData::internal_error(
                                    format!("Failed to read entry: {}", e),
                                    None,
                                )
                            })?;
                            let path = entry.path();
                            if path.is_file() && path.extension() == Some("pub".as_ref()) {
                                let basename = path
                                    .file_name()
                                    .ok_or_else(|| {
                                        ErrorData::internal_error(
                                            format!(
                                                "Cannot get base name for file {}",
                                                path.display()
                                            ),
                                            None,
                                        )
                                    })?
                                    .to_str()
                                    .ok_or_else(|| {
                                        ErrorData::internal_error(
                                            format!(
                                                "Cannot convert basename to string for file {}",
                                                path.display()
                                            ),
                                            None,
                                        )
                                    })?;

                                public_keys.push(String::from(basename));
                            }
                        }

                        Ok(ReadResourceResult {
                            contents: vec![ResourceContents::text(
                                public_keys.join(","),
                                request.uri.to_string(),
                            )],
                        })
                    }
                    _ => {
                        return Err(ErrorData::invalid_request(
                            format!("Invalid path: {}", path_str),
                            None,
                        ));
                    }
                }
            }
            _ => {
                return Err(ErrorData::invalid_request(
                    format!(
                        "Invalid URI scheme. Supported schemes: file://, got: {}",
                        url.scheme()
                    ),
                    None,
                ));
            }
        }
    }
}
