use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, Diagnostic, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentSymbol, DocumentSymbolParams, Hover, HoverContents,
    HoverParams, MarkedString, NumberOrString, OneOf, Position, PublishDiagnosticsParams, Range,
    ServerCapabilities, SymbolKind, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, TextDocumentSyncSaveOptions, TextEdit, Uri,
};
use serde_json::Value;
use url::Url;

use crate::analysis::{self, AnalysisResult, SourceOverlays};
use crate::compiler::FunctionInfo;
use crate::errors::{ErrorCode, ErrorPhase, KiroError};
use crate::formatter;
use crate::grammar::{self, grammar as ast};

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
        document_symbol_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        ..ServerCapabilities::default()
    }
}

#[derive(Default)]
struct LspState {
    documents: HashMap<Uri, OpenDocument>,
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
                self.publish_diagnostics(connection, &params.text_document.uri)?;
            }
            "textDocument/didClose" => {
                let params: DidCloseTextDocumentParams =
                    serde_json::from_value(notification.params).map_err(lsp_error)?;
                self.documents.remove(&params.text_document.uri);
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
        let mut overlays = SourceOverlays::new();
        overlays.insert(document.path.clone(), document.text.clone());
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

    fn hover(&self, params: Value) -> Result<Value, KiroError> {
        let params: HoverParams = serde_json::from_value(params).map_err(lsp_error)?;
        let doc = params.text_document_position_params;
        let Some((_path, source)) = self.source_for_uri(&doc.text_document.uri) else {
            return Ok(Value::Null);
        };
        let Some(word) = word_at(&source, doc.position) else {
            return Ok(Value::Null);
        };
        let Some(text) = hover_doc(&word) else {
            return Ok(Value::Null);
        };
        let hover = Hover {
            contents: HoverContents::Scalar(MarkedString::String(text.to_string())),
            range: None,
        };
        serde_json::to_value(hover).map_err(lsp_error)
    }

    fn completion(&self, params: Value) -> Result<Value, KiroError> {
        let params: lsp_types::CompletionParams =
            serde_json::from_value(params).map_err(lsp_error)?;
        let doc = params.text_document_position;
        let Some((path, source)) = self.source_for_uri(&doc.text_document.uri) else {
            return serde_json::to_value(Vec::<CompletionItem>::new()).map_err(lsp_error);
        };

        let mut items = if let Some(module) = module_prefix_at(&source, doc.position) {
            module_completion_items(&path, &source, &module)
        } else {
            general_completion_items(&source)
        };
        items.sort_by(|a, b| a.label.cmp(&b.label));
        items.dedup_by(|a, b| a.label == b.label);

        serde_json::to_value(items).map_err(lsp_error)
    }

    fn document_symbols(&self, params: Value) -> Result<Value, KiroError> {
        let params: DocumentSymbolParams = serde_json::from_value(params).map_err(lsp_error)?;
        let Some((_path, source)) = self.source_for_uri(&params.text_document.uri) else {
            return serde_json::to_value(Vec::<DocumentSymbol>::new()).map_err(lsp_error);
        };
        let program = match grammar::parse(&source) {
            Ok(program) => program,
            Err(_) => return serde_json::to_value(Vec::<DocumentSymbol>::new()).map_err(lsp_error),
        };
        serde_json::to_value(document_symbols_from_program(&program, &source)).map_err(lsp_error)
    }

    fn source_for_uri(&self, uri: &Uri) -> Option<(PathBuf, String)> {
        if let Some(document) = self.documents.get(uri) {
            return Some((document.path.clone(), document.text.clone()));
        }
        let path = uri_to_path(uri)?;
        let source = std::fs::read_to_string(&path).ok()?;
        Some((path, source))
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

fn general_completion_items(source: &str) -> Vec<CompletionItem> {
    let mut labels = BTreeMap::new();
    for keyword in KEYWORDS {
        labels.insert(
            (*keyword).to_string(),
            (CompletionItemKind::KEYWORD, "keyword".to_string()),
        );
    }
    if let Ok(program) = grammar::parse(source) {
        collect_program_completion_labels(&program, &mut labels);
    }
    labels
        .into_iter()
        .map(|(label, (kind, detail))| completion_item(&label, kind, &detail))
        .collect()
}

fn module_completion_items(path: &Path, source: &str, module: &str) -> Vec<CompletionItem> {
    let mut overlays = SourceOverlays::new();
    overlays.insert(path.to_path_buf(), source.to_string());
    let Ok(result) = analysis::analyze_path_with_info(path, &overlays) else {
        return Vec::new();
    };
    module_function_items(&result, module)
}

fn module_function_items(result: &AnalysisResult, module: &str) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    for ((module_name, function_name), info) in &result.module_functions {
        if module_name == module {
            items.push(completion_item(
                function_name,
                CompletionItemKind::FUNCTION,
                &function_detail(info),
            ));
        }
    }
    items
}

fn collect_program_completion_labels(
    program: &ast::Program,
    labels: &mut BTreeMap<String, (CompletionItemKind, String)>,
) {
    for stmt in &program.statements {
        match stmt {
            ast::Statement::Import { module_name, .. } => {
                labels.insert(
                    module_name.clone(),
                    (CompletionItemKind::MODULE, "module".to_string()),
                );
            }
            ast::Statement::FunctionDef(def) => {
                labels.insert(
                    def.name.clone(),
                    (CompletionItemKind::FUNCTION, "function".to_string()),
                );
            }
            ast::Statement::RustFnDecl(def) => {
                labels.insert(
                    def.name.clone(),
                    (CompletionItemKind::FUNCTION, "rust fn".to_string()),
                );
            }
            ast::Statement::VarDecl { ident, .. } => {
                labels.insert(
                    ident.clone(),
                    (CompletionItemKind::VARIABLE, "variable".to_string()),
                );
            }
            ast::Statement::Documented { item, .. } => match item {
                ast::AnnotatableItem::FunctionDef(def) => {
                    labels.insert(
                        def.name.clone(),
                        (CompletionItemKind::FUNCTION, "function".to_string()),
                    );
                }
                ast::AnnotatableItem::RustFnDecl(def) => {
                    labels.insert(
                        def.name.clone(),
                        (CompletionItemKind::FUNCTION, "rust fn".to_string()),
                    );
                }
                ast::AnnotatableItem::StructDef(def) => {
                    labels.insert(
                        def.name.value.clone(),
                        (CompletionItemKind::STRUCT, "struct".to_string()),
                    );
                }
            },
            ast::Statement::StructDef(def) => {
                labels.insert(
                    def.name.value.clone(),
                    (CompletionItemKind::STRUCT, "struct".to_string()),
                );
            }
            _ => {}
        }
    }
}

fn completion_item(label: &str, kind: CompletionItemKind, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: Some(detail.to_string()),
        ..CompletionItem::default()
    }
}

fn function_detail(info: &FunctionInfo) -> String {
    let purity = if info.is_pure { "pure fn" } else { "fn" };
    let failable = if info.can_error { "!" } else { "" };
    format!("{} -> {:?}{}", purity, info.return_type, failable)
}

fn document_symbols_from_program(program: &ast::Program, source: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    for stmt in &program.statements {
        match stmt {
            ast::Statement::Import { module_name, .. } => {
                symbols.push(symbol(module_name, "import", SymbolKind::MODULE, source));
            }
            ast::Statement::FunctionDef(def) => {
                let detail = if def.pure_kw.is_some() {
                    "pure fn"
                } else {
                    "fn"
                };
                symbols.push(symbol(&def.name, detail, SymbolKind::FUNCTION, source));
            }
            ast::Statement::RustFnDecl(def) => {
                symbols.push(symbol(&def.name, "rust fn", SymbolKind::FUNCTION, source));
            }
            ast::Statement::StructDef(def) => {
                symbols.push(symbol(
                    &def.name.value,
                    "struct",
                    SymbolKind::STRUCT,
                    source,
                ));
            }
            ast::Statement::ErrorDef { name, .. } => {
                symbols.push(symbol(name, "error", SymbolKind::ENUM_MEMBER, source));
            }
            ast::Statement::Documented { item, .. } => match item {
                ast::AnnotatableItem::FunctionDef(def) => {
                    symbols.push(symbol(&def.name, "fn", SymbolKind::FUNCTION, source));
                }
                ast::AnnotatableItem::RustFnDecl(def) => {
                    symbols.push(symbol(&def.name, "rust fn", SymbolKind::FUNCTION, source));
                }
                ast::AnnotatableItem::StructDef(def) => {
                    symbols.push(symbol(
                        &def.name.value,
                        "struct",
                        SymbolKind::STRUCT,
                        source,
                    ));
                }
            },
            _ => {}
        }
    }
    symbols
}

#[allow(deprecated)]
fn symbol(name: &str, detail: &str, kind: SymbolKind, source: &str) -> DocumentSymbol {
    let range = range_for_token(source, name);
    DocumentSymbol {
        name: name.to_string(),
        detail: Some(detail.to_string()),
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

fn range_for_token(source: &str, token: &str) -> Range {
    for (line_idx, line) in source.lines().enumerate() {
        if let Some(col) = line.find(token) {
            return Range {
                start: Position {
                    line: line_idx as u32,
                    character: col as u32,
                },
                end: Position {
                    line: line_idx as u32,
                    character: (col + token.len()).max(col + 1) as u32,
                },
            };
        }
    }
    Range::default()
}

fn full_range(source: &str) -> Range {
    let line_count = source.lines().count() as u32;
    Range {
        start: Position::new(0, 0),
        end: Position::new(line_count.saturating_add(1), 0),
    }
}

fn word_at(source: &str, position: Position) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let char_idx = position.character as usize;
    let bytes = line.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut idx = char_idx.min(bytes.len().saturating_sub(1));
    if !is_word_byte(bytes[idx]) && idx > 0 {
        idx -= 1;
    }
    if !is_word_byte(bytes[idx]) {
        return None;
    }
    let mut start = idx;
    while start > 0 && is_word_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = idx + 1;
    while end < bytes.len() && is_word_byte(bytes[end]) {
        end += 1;
    }
    Some(line[start..end].to_string())
}

fn module_prefix_at(source: &str, position: Position) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let prefix = line.get(..(position.character as usize).min(line.len()))?;
    let before_dot = prefix.strip_suffix('.')?;
    let module = before_dot
        .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .next()?;
    if module.is_empty() {
        None
    } else {
        Some(module.to_string())
    }
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn hover_doc(word: &str) -> Option<&'static str> {
    match word {
        "fn" => Some("Defines a function. Normal functions are async in generated Rust."),
        "pure" => Some("Marks a function as deterministic and effect-free."),
        "rust" => Some("Declares a host function implemented by adjacent Rust glue."),
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
        "std_fs" => Some("Standard filesystem host module."),
        "std_io" => Some("Standard input/output host module."),
        "std_time" => Some("Standard time host module."),
        "std_net" => Some("Standard network host module."),
        "std_env" => Some("Standard environment host module."),
        _ => None,
    }
}

const KEYWORDS: &[&str] = &[
    "adr", "break", "check", "close", "continue", "error", "fn", "give", "import", "in", "loop",
    "map", "move", "off", "on", "pipe", "print", "pure", "rest", "return", "run", "rust", "struct",
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
