use zed_extension_api::{self as zed, Command, LanguageServerId, Worktree};

struct KiroExtension;

impl zed::Extension for KiroExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> zed::Result<Command> {
        let binary = worktree
            .which("kiro-lang")
            .or_else(|| worktree.which("kiro"))
            .unwrap_or_else(|| "kiro-lang".to_string());

        Ok(Command {
            command: binary,
            args: vec!["lsp".to_string()],
            env: Vec::new(),
        })
    }
}

zed::register_extension!(KiroExtension);
