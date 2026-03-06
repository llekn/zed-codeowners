mod codeowners;

use std::path::PathBuf;
use std::sync::RwLock;

use codeowners::Codeowners;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    workspace_root: RwLock<Option<PathBuf>>,
    codeowners: RwLock<Option<Codeowners>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            workspace_root: RwLock::new(None),
            codeowners: RwLock::new(None),
        }
    }

    fn load_codeowners(&self) {
        let root = self.workspace_root.read().unwrap();
        if let Some(root) = root.as_ref() {
            let co = Codeowners::from_workspace(root);
            *self.codeowners.write().unwrap() = co;
        }
    }

    fn diagnose_file(&self, uri: &Url) -> Vec<Diagnostic> {
        let file_path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return vec![],
        };

        let root = self.workspace_root.read().unwrap();
        let root = match root.as_ref() {
            Some(r) => r,
            None => return vec![],
        };

        let relative = match file_path.strip_prefix(root) {
            Ok(r) => r.to_string_lossy().to_string(),
            Err(_) => return vec![],
        };

        let co = self.codeowners.read().unwrap();
        let co = match co.as_ref() {
            Some(c) => c,
            None => return vec![],
        };

        let message = match co.owners_of(&relative) {
            Some(owners) => format!("Owner: {}", owners.join(", ")),
            None => return vec![],
        };

        vec![Diagnostic {
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 0),
            },
            severity: Some(DiagnosticSeverity::HINT),
            source: Some("codeowners".to_string()),
            message,
            ..Default::default()
        }]
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(root_uri) = params.root_uri {
            if let Ok(path) = root_uri.to_file_path() {
                *self.workspace_root.write().unwrap() = Some(path);
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        change: Some(TextDocumentSyncKind::NONE),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.load_codeowners();
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let diagnostics = self.diagnose_file(&params.text_document.uri);
        self.client
            .publish_diagnostics(params.text_document.uri, diagnostics, None)
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        // If the CODEOWNERS file itself was saved, reload it
        if let Ok(path) = params.text_document.uri.to_file_path() {
            if path.file_name().map(|n| n == "CODEOWNERS").unwrap_or(false) {
                self.load_codeowners();
            }
        }

        let diagnostics = self.diagnose_file(&params.text_document.uri);
        self.client
            .publish_diagnostics(params.text_document.uri, diagnostics, None)
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
