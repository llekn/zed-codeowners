use zed_extension_api::{self as zed, Result};
use std::fs;

macro_rules! debug_log {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        eprintln!($($arg)*);
    };
}

struct CodeownersExtension {
    cached_binary_path: Option<String>,
}

impl CodeownersExtension {
    fn language_server_binary_path(&mut self, worktree: &zed::Worktree) -> Result<String> {
        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).map(|m| m.is_file()).unwrap_or(false) {
                debug_log!("[codeowners-ext] Using cached binary: {}", path);
                return Ok(path.clone());
            }
        }

        // Try to find codeowners-lsp in PATH
        if let Some(path) = worktree.which("codeowners-lsp") {
            debug_log!("[codeowners-ext] Found binary at: {}", path);
            self.cached_binary_path = Some(path.clone());
            return Ok(path);
        }

        Err("codeowners-lsp binary not found in PATH. Please install it: cargo install --path codeowners-lsp".into())
    }
}

impl zed::Extension for CodeownersExtension {
    fn new() -> Self {
        debug_log!("[codeowners-ext] Extension initialized");
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        debug_log!("[codeowners-ext] language_server_command called for: {:?}", _language_server_id);
        let binary_path = self.language_server_binary_path(worktree)?;
        Ok(zed::Command {
            command: binary_path,
            args: vec![],
            env: Default::default(),
        })
    }
}

zed::register_extension!(CodeownersExtension);
