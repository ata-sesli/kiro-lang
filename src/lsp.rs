use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, Diagnostic, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentSymbol, DocumentSymbolParams, GotoDefinitionParams, Hover,
    HoverContents, HoverParams, MarkedString, NumberOrString, OneOf, Position,
    PublishDiagnosticsParams, Range, ServerCapabilities, SignatureHelpOptions, SignatureHelpParams,
    SymbolKind, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, TextEdit, Uri,
};
use serde_json::Value;
use url::Url;

use crate::analysis::{self, SourceOverlays};
use crate::errors::{ErrorCode, ErrorPhase, KiroError};
use crate::formatter;
use crate::lsp_symbols::{self, IndexedKind, SymbolDecl, SymbolIndex};

pub fn run() -> Result<(), KiroError> {
    let (connection, io_threads) = Connection::stdio();
    let capabilities = serde_json::to_value(server_capabilities()).map_err(lsp_error)?;
    connection.initialize(capabilities).map_err(lsp_error)?;

    let mut server = LspState::default();
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request).map_err(lsp_error)? {
                    break;
                }
                server.handle_request(&connection, request)?;
            }
            Message::Notification(notification) => {
                server.handle_notification(&connection, notification)?;
            }
            Message::Response(_) => {}
        }
    }

    drop(connection);
    io_threads.join().map_err(lsp_error)?;
    Ok(())
}

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                will_save: None,
                will_save_wait_until: None,
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(
                    lsp_types::SaveOptions {
                        include_text: Some(true),
                    },
                )),
            },
        )),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![".".to_string()]),
            ..CompletionOptions::default()
        }),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
            retrigger_characters: Some(vec![",".to_string()]),
            work_done_progress_options: Default::default(),
        }),
        definition_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        ..ServerCapabilities::default()
    }
}

#[derive(Default)]
struct LspState {
    documents: HashMap<Uri, OpenDocument>,
    symbol_cache: HashMap<PathBuf, SymbolIndex>,
}

#[derive(Clone)]
struct OpenDocument {
    path: PathBuf,
    text: String,
    version: Option<i32>,
}

impl LspState {
    fn handle_request(
        &mut self,
        connection: &Connection,
        request: Request,
    ) -> Result<(), KiroError> {
        let id = request.id.clone();
        let result = match request.method.as_str() {
            "textDocument/formatting" => self.formatting(request.params),
            "textDocument/hover" => self.hover(request.params),
            "textDocument/completion" => self.completion(request.params),
            "textDocument/documentSymbol" => self.document_symbols(request.params),
            "textDocument/definition" => self.definition(request.params),
            "textDocument/signatureHelp" => self.signature_help(request.params),
            _ => {
                let response = Response::new_err(
                    id,
                    lsp_server::ErrorCode::MethodNotFound as i32,
                    format!("Unsupported request '{}'.", request.method),
                );
                connection.sender.send(response.into()).map_err(lsp_error)?;
                return Ok(());
            }
        };

        match result {
            Ok(result) => {
                connection
                    .sender
                    .send(Response::new_ok(id, result).into())
                    .map_err(lsp_error)?;
            }
            Err(err) => {
                connection
                    .sender
                    .send(
                        Response::new_err(
                            id,
                            lsp_server::ErrorCode::RequestFailed as i32,
                            err.message,
                        )
                        .into(),
                    )
                    .map_err(lsp_error)?;
            }
        }
        Ok(())
    }

    fn handle_notification(
        &mut self,
        connection: &Connection,
        notification: Notification,
    ) -> Result<(), KiroError> {
        match notification.method.as_str() {
            "initialized" => {}
            "textDocument/didOpen" => {
                let params: DidOpenTextDocumentParams =
                    serde_json::from_value(notification.params).map_err(lsp_error)?;
                if let Some(path) = uri_to_path(&params.text_document.uri) {
                    self.documents.insert(
                        params.text_document.uri,
                        OpenDocument {
                            path,
                            text: params.text_document.text,
                            version: Some(params.text_document.version),
                        },
                    );
                    self.clear_symbol_cache();
                }
            }
            "textDocument/didChange" => {
                let params: DidChangeTextDocumentParams =
                    serde_json::from_value(notification.params).map_err(lsp_error)?;
                if let Some(document) = self.documents.get_mut(&params.text_document.uri)
                    && let Some(change) = params.content_changes.into_iter().last()
                {
                    document.text = change.text;
                    document.version = Some(params.text_document.version);
                    self.clear_symbol_cache();
                }
            }
            "textDocument/didSave" => {
                let params: DidSaveTextDocumentParams =
                    serde_json::from_value(notification.params).map_err(lsp_error)?;
                if let Some(text) = params.text
                    && let Some(document) = self.documents.get_mut(&params.text_document.uri)
                {
                    document.text = text;
                }
                self.clear_symbol_cache();
                self.publish_diagnostics(connection, &params.text_document.uri)?;
            }
            "textDocument/didClose" => {
                let params: DidCloseTextDocumentParams =
                    serde_json::from_value(notification.params).map_err(lsp_error)?;
                self.documents.remove(&params.text_document.uri);
                self.clear_symbol_cache();
                publish(connection, params.text_document.uri, Vec::new(), None)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn publish_diagnostics(&self, connection: &Connection, uri: &Uri) -> Result<(), KiroError> {
        let Some(document) = self.documents.get(uri) else {
            return Ok(());
        };
        let overlays = self.source_overlays();
        let diagnostics = match analysis::analyze_path(&document.path, &overlays) {
            Ok(()) => Vec::new(),
            Err(err) => vec![diagnostic_from_error(err)],
        };
        publish(connection, uri.clone(), diagnostics, document.version)
    }

    fn formatting(&self, params: Value) -> Result<Value, KiroError> {
        let params: lsp_types::DocumentFormattingParams =
            serde_json::from_value(params).map_err(lsp_error)?;
        let Some((path, source)) = self.source_for_uri(&params.text_document.uri) else {
            return Ok(Value::Null);
        };
        let formatted = formatter::format_source_for_file(&source, &display_name(&path))?;
        let edits = if formatted == source {
            Vec::<TextEdit>::new()
        } else {
            vec![TextEdit::new(full_range(&source), formatted)]
        };
        serde_json::to_value(edits).map_err(lsp_error)
    }

    fn definition(&mut self, params: Value) -> Result<Value, KiroError> {
        let params: GotoDefinitionParams = serde_json::from_value(params).map_err(lsp_error)?;
        let doc = params.text_document_position_params;
        let Some((path, source)) = self.source_for_uri(&doc.text_document.uri) else {
            return Ok(Value::Null);
        };
        let index = self.symbol_index_for(&path, &source);
        let Some(location) = index.definition_at(&path, &source, doc.position) else {
            return Ok(Value::Null);
        };
        serde_json::to_value(location).map_err(lsp_error)
    }

    fn hover(&mut self, params: Value) -> Result<Value, KiroError> {
        let params: HoverParams = serde_json::from_value(params).map_err(lsp_error)?;
        let doc = params.text_document_position_params;
        let Some((path, source)) = self.source_for_uri(&doc.text_document.uri) else {
            return Ok(Value::Null);
        };
        let index = self.symbol_index_for(&path, &source);
        let text = index.hover_at(&path, &source, doc.position).or_else(|| {
            lsp_symbols::word_at(&source, doc.position)
                .and_then(|word| hover_doc(&word).map(str::to_string))
        });
        let Some(text) = text else {
            return Ok(Value::Null);
        };
        let hover = Hover {
            contents: HoverContents::Scalar(MarkedString::String(text)),
            range: None,
        };
        serde_json::to_value(hover).map_err(lsp_error)
    }

    fn completion(&mut self, params: Value) -> Result<Value, KiroError> {
        let params: lsp_types::CompletionParams =
            serde_json::from_value(params).map_err(lsp_error)?;
        let doc = params.text_document_position;
        let Some((path, source)) = self.source_for_uri(&doc.text_document.uri) else {
            return serde_json::to_value(Vec::<CompletionItem>::new()).map_err(lsp_error);
        };

        let mut items = if lsp_symbols::is_import_completion(&source, doc.position) {
            import_completion_items(&path)
        } else {
            let index = self.symbol_index_for(&path, &source);
            if let Some(module) = lsp_symbols::module_prefix_at(&source, doc.position) {
                let mut items = module_completion_items(&index, &module);
                if items.is_empty() {
                    items = self
                        .index_for_sibling_module(&path, &module)
                        .map(|index| module_completion_items(&index, &module))
                        .unwrap_or_default();
                }
                items
            } else {
                general_completion_items(&index, &path)
            }
        };
        items.sort_by(|a, b| a.label.cmp(&b.label));
        items.dedup_by(|a, b| a.label == b.label);

        serde_json::to_value(items).map_err(lsp_error)
    }

    fn signature_help(&mut self, params: Value) -> Result<Value, KiroError> {
        let params: SignatureHelpParams = serde_json::from_value(params).map_err(lsp_error)?;
        let doc = params.text_document_position_params;
        let Some((path, source)) = self.source_for_uri(&doc.text_document.uri) else {
            return Ok(Value::Null);
        };
        let index = self.symbol_index_for(&path, &source);
        let Some(help) = index.signature_help_at(&path, &source, doc.position) else {
            return Ok(Value::Null);
        };
        serde_json::to_value(help).map_err(lsp_error)
    }

    fn document_symbols(&mut self, params: Value) -> Result<Value, KiroError> {
        let params: DocumentSymbolParams = serde_json::from_value(params).map_err(lsp_error)?;
        let Some((path, source)) = self.source_for_uri(&params.text_document.uri) else {
            return serde_json::to_value(Vec::<DocumentSymbol>::new()).map_err(lsp_error);
        };
        let index = self.symbol_index_for(&path, &source);
        serde_json::to_value(document_symbols_from_index(&index, &path)).map_err(lsp_error)
    }

    fn source_for_uri(&self, uri: &Uri) -> Option<(PathBuf, String)> {
        if let Some(document) = self.documents.get(uri) {
            return Some((document.path.clone(), document.text.clone()));
        }
        let path = uri_to_path(uri)?;
        let source = std::fs::read_to_string(&path).ok()?;
        Some((path, source))
    }

    fn source_overlays(&self) -> SourceOverlays {
        self.documents
            .values()
            .map(|document| (document.path.clone(), document.text.clone()))
            .collect()
    }

    fn symbol_index_for(&mut self, path: &Path, source: &str) -> SymbolIndex {
        if let Some(index) = self.symbol_cache.get(path) {
            return index.clone();
        }
        let mut overlays = self.source_overlays();
        overlays.insert(path.to_path_buf(), source.to_string());
        let index = SymbolIndex::build(path, &overlays);
        self.symbol_cache.insert(path.to_path_buf(), index.clone());
        index
    }

    fn index_for_sibling_module(&mut self, path: &Path, module: &str) -> Option<SymbolIndex> {
        let module_path = path.parent()?.join(format!("{}.kiro", module));
        if let Some(index) = self.symbol_cache.get(&module_path) {
            return Some(index.clone());
        }
        let overlays = self.source_overlays();
        let index = SymbolIndex::build(&module_path, &overlays);
        self.symbol_cache.insert(module_path, index.clone());
        Some(index)
    }

    fn clear_symbol_cache(&mut self) {
        self.symbol_cache.clear();
    }
}

fn publish(
    connection: &Connection,
    uri: Uri,
    diagnostics: Vec<Diagnostic>,
    version: Option<i32>,
) -> Result<(), KiroError> {
    let params = PublishDiagnosticsParams::new(uri, diagnostics, version);
    connection
        .sender
        .send(
            Notification::new(
                "textDocument/publishDiagnostics".to_string(),
                serde_json::to_value(params).map_err(lsp_error)?,
            )
            .into(),
        )
        .map_err(lsp_error)
}

fn diagnostic_from_error(err: KiroError) -> Diagnostic {
    let line = err.line.unwrap_or(1).saturating_sub(1) as u32;
    let column = err.column.unwrap_or(1).saturating_sub(1) as u32;
    let mut message = err.message;
    if let Some(help) = err.help {
        message.push_str(&format!("\nhelp: {}", help));
    }
    if let Some(suggestion) = err.suggestion {
        message.push_str(&format!("\nhelp: {}", suggestion));
    }

    Diagnostic::new(
        Range {
            start: Position {
                line,
                character: column,
            },
            end: Position {
                line,
                character: column.saturating_add(1),
            },
        },
        Some(DiagnosticSeverity::ERROR),
        Some(NumberOrString::String(err.code.to_string())),
        Some("kiro".to_string()),
        message,
        None,
        None,
    )
}

fn import_completion_items(path: &Path) -> Vec<CompletionItem> {
    lsp_symbols::sibling_and_std_modules(path)
        .into_iter()
        .map(|module| completion_item(&module, CompletionItemKind::MODULE, "module"))
        .collect()
}

fn general_completion_items(index: &SymbolIndex, path: &Path) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    for keyword in KEYWORDS {
        items.push(completion_item(
            keyword,
            CompletionItemKind::KEYWORD,
            "keyword",
        ));
    }
    for decl in index.declarations_for_path(path) {
        items.push(completion_item(
            &decl.name,
            completion_kind(&decl.kind),
            &completion_detail(decl),
        ));
    }
    items
}

fn module_completion_items(index: &SymbolIndex, module: &str) -> Vec<CompletionItem> {
    let mut items = index
        .module_functions(module)
        .into_iter()
        .map(|decl| {
            completion_item(
                &decl.name,
                completion_kind(&decl.kind),
                &completion_detail(decl),
            )
        })
        .collect::<Vec<_>>();
    if crate::is_std_io_module_name(module) {
        for name in ["print", "write", "eprint", "eprintline"] {
            if !items.iter().any(|item| item.label == name) {
                let detail = lsp_symbols::std_io_display_signature_label(module, name)
                    .unwrap_or_else(|| "std io display function".to_string());
                items.push(completion_item(name, CompletionItemKind::FUNCTION, &detail));
            }
        }
    }
    items
}

fn completion_kind(kind: &IndexedKind) -> CompletionItemKind {
    match kind {
        IndexedKind::Import => CompletionItemKind::MODULE,
        IndexedKind::Function | IndexedKind::RustFunction => CompletionItemKind::FUNCTION,
        IndexedKind::Handle => CompletionItemKind::STRUCT,
        IndexedKind::Struct => CompletionItemKind::STRUCT,
        IndexedKind::Error => CompletionItemKind::ENUM_MEMBER,
        IndexedKind::Var => CompletionItemKind::VARIABLE,
    }
}

fn completion_detail(decl: &SymbolDecl) -> String {
    decl.signature
        .clone()
        .unwrap_or_else(|| decl.detail.clone())
}

fn completion_item(label: &str, kind: CompletionItemKind, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: Some(detail.to_string()),
        ..CompletionItem::default()
    }
}

fn document_symbols_from_index(index: &SymbolIndex, path: &Path) -> Vec<DocumentSymbol> {
    index
        .declarations_for_path(path)
        .into_iter()
        .map(symbol_from_decl)
        .collect()
}

#[allow(deprecated)]
fn symbol_from_decl(decl: &SymbolDecl) -> DocumentSymbol {
    DocumentSymbol {
        name: decl.name.clone(),
        detail: Some(completion_detail(decl)),
        kind: symbol_kind(&decl.kind),
        tags: None,
        deprecated: None,
        range: decl.range,
        selection_range: decl.selection_range,
        children: None,
    }
}

fn symbol_kind(kind: &IndexedKind) -> SymbolKind {
    match kind {
        IndexedKind::Import => SymbolKind::MODULE,
        IndexedKind::Function | IndexedKind::RustFunction => SymbolKind::FUNCTION,
        IndexedKind::Handle => SymbolKind::STRUCT,
        IndexedKind::Struct => SymbolKind::STRUCT,
        IndexedKind::Error => SymbolKind::ENUM_MEMBER,
        IndexedKind::Var => SymbolKind::VARIABLE,
    }
}

fn full_range(source: &str) -> Range {
    let line_count = source.lines().count() as u32;
    Range {
        start: Position::new(0, 0),
        end: Position::new(line_count.saturating_add(1), 0),
    }
}

fn hover_doc(word: &str) -> Option<&'static str> {
    match word {
        "fn" => Some("Defines a function. Normal functions are async in generated Rust."),
        "pure" => Some("Marks a function as deterministic and effect-free."),
        "rust" => Some("Declares a host function implemented by adjacent Rust glue."),
        "handle" => Some("Declares an opaque host-owned value type."),
        "run" => Some("Starts a function as fire-and-forget work."),
        "rest" => {
            Some("Gives other running tasks a chance to continue; does not send data or sleep.")
        }
        "check" => Some(
            "Checks that a condition is true; failed checks stop the program with a Kiro diagnostic.",
        ),
        "pipe" => Some("Creates a typed communication pipe."),
        "give" => Some("Sends a value into a pipe."),
        "take" => Some("Receives a value from a pipe."),
        "close" => Some("Closes a pipe sender."),
        "on" => Some("Starts a condition or error-handling block."),
        "off" => Some("Runs the alternative branch for an on block."),
        "import" => Some("Imports a sibling Kiro module or embedded std module."),
        "fs" | "std_fs" => Some("Standard filesystem host module."),
        "io" | "std_io" => Some("Standard input/output host module."),
        "time" | "std_time" => Some("Standard time host module."),
        "net" | "std_net" => Some("Standard network host module."),
        "env" | "std_env" => Some("Standard environment host module."),
        _ => None,
    }
}

const KEYWORDS: &[&str] = &[
    "adr", "break", "check", "close", "continue", "error", "fn", "give", "handle", "import", "in",
    "loop", "map", "move", "off", "on", "pipe", "pure", "rest", "return", "run", "rust", "struct",
    "take", "var",
];

fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    Url::parse(uri.as_str()).ok()?.to_file_path().ok()
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

fn lsp_error(err: impl std::fmt::Display) -> KiroError {
    KiroError::new(
        ErrorCode::BuildGraphFailed,
        ErrorPhase::Cli,
        format!("LSP error: {}", err),
    )
}

#[allow(dead_code)]
fn uri_from_str(raw: &str) -> Result<Uri, KiroError> {
    Uri::from_str(raw).map_err(lsp_error)
}
