use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use lsp_types::{
    Documentation, Location, ParameterInformation, ParameterLabel, Position, Range, SignatureHelp,
    SignatureInformation, Uri,
};
use url::Url;

use crate::analysis::{self, AnalysisResult, SourceOverlays};
use crate::grammar::{self, grammar as ast};

pub const STD_MODULES: &[&str] = &["std_fs", "std_io", "std_time", "std_net", "std_env"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexedKind {
    Import,
    Function,
    RustFunction,
    Struct,
    Error,
    Var,
}

#[derive(Clone, Debug)]
pub struct SymbolDecl {
    pub name: String,
    pub kind: IndexedKind,
    pub module: String,
    pub path: PathBuf,
    pub range: Range,
    pub selection_range: Range,
    pub detail: String,
    pub signature: Option<String>,
    pub params: Vec<String>,
    pub doc: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ModuleSymbols {
    pub name: String,
    pub path: PathBuf,
    pub source: String,
    pub declarations: Vec<SymbolDecl>,
}

#[derive(Clone, Debug, Default)]
pub struct SymbolIndex {
    modules: HashMap<String, ModuleSymbols>,
}

impl SymbolIndex {
    pub fn build(path: &Path, overlays: &SourceOverlays) -> Self {
        match analysis::analyze_path_with_info(path, overlays) {
            Ok(result) => Self::from_analysis(result),
            Err(_) => Self::parse_only(path, overlays),
        }
    }

    pub fn declarations_for_path(&self, path: &Path) -> Vec<&SymbolDecl> {
        let normalized = normalize_path(path);
        self.modules
            .values()
            .find(|module| normalize_path(&module.path) == normalized)
            .map(|module| module.declarations.iter().collect())
            .unwrap_or_default()
    }

    pub fn module_functions(&self, module_name: &str) -> Vec<&SymbolDecl> {
        self.modules
            .get(module_name)
            .map(|module| {
                module
                    .declarations
                    .iter()
                    .filter(|decl| {
                        matches!(decl.kind, IndexedKind::Function | IndexedKind::RustFunction)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn module_names(&self) -> Vec<String> {
        let mut names = self.modules.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }

    pub fn definition_at(&self, path: &Path, source: &str, position: Position) -> Option<Location> {
        if let Some((module, member)) = member_access_at(source, position) {
            let decl = self.find_module_member(&module, &member)?;
            return location_for(decl);
        }

        let word = word_at(source, position)?;
        let current = self.module_for_path(path)?;
        if let Some(decl) = current.declarations.iter().find(|decl| decl.name == word) {
            if decl.kind == IndexedKind::Import {
                if let Some(module) = self.modules.get(&decl.name) {
                    return module_location(module);
                }
            }
            return location_for(decl);
        }

        if let Some(module) = self.modules.get(&word) {
            return module_location(module);
        }

        None
    }

    pub fn hover_at(&self, path: &Path, source: &str, position: Position) -> Option<String> {
        if let Some((module, member)) = member_access_at(source, position) {
            return self.find_module_member(&module, &member).map(hover_text);
        }

        let word = word_at(source, position)?;
        let current = self.module_for_path(path)?;
        if let Some(decl) = current.declarations.iter().find(|decl| decl.name == word) {
            return Some(hover_text(decl));
        }
        if self.modules.contains_key(&word) {
            return Some(format!("module {}", word));
        }
        None
    }

    pub fn signature_help_at(
        &self,
        path: &Path,
        source: &str,
        position: Position,
    ) -> Option<SignatureHelp> {
        let ctx = call_context_at(source, position)?;
        let decl = if let Some((module, member)) = ctx.call_name.split_once('.') {
            self.find_module_member(module, member)?
        } else {
            let current = self.module_for_path(path)?;
            current
                .declarations
                .iter()
                .find(|decl| decl.name == ctx.call_name && decl.signature.is_some())?
        };
        let signature = decl.signature.clone()?;
        let parameters = if decl.params.is_empty() {
            None
        } else {
            Some(
                decl.params
                    .iter()
                    .cloned()
                    .map(|param| ParameterInformation {
                        label: ParameterLabel::Simple(param),
                        documentation: None,
                    })
                    .collect(),
            )
        };
        let active_parameter = ctx
            .active_parameter
            .min(decl.params.len().saturating_sub(1))
            .try_into()
            .ok();
        Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label: signature,
                documentation: decl.doc.clone().map(Documentation::String),
                parameters,
                active_parameter: None,
            }],
            active_signature: Some(0),
            active_parameter,
        })
    }

    fn from_analysis(result: AnalysisResult) -> Self {
        let mut modules = HashMap::new();
        for module in result.modules.into_values() {
            modules.insert(
                module.name.clone(),
                index_module(&module.name, module.path, module.source, &module.program),
            );
        }
        Self { modules }
    }

    fn parse_only(path: &Path, overlays: &SourceOverlays) -> Self {
        let normalized = normalize_path(path);
        let source = overlays
            .get(&normalized)
            .cloned()
            .or_else(|| std::fs::read_to_string(&normalized).ok());
        let Some(source) = source else {
            return Self::default();
        };
        let name = normalized
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("main")
            .to_string();
        let module = match grammar::parse(&source) {
            Ok(program) => index_module(&name, normalized, source, &program),
            Err(_) => index_module_line_first(&name, normalized, source),
        };
        let mut modules = HashMap::new();
        modules.insert(name, module);
        Self { modules }
    }

    fn module_for_path(&self, path: &Path) -> Option<&ModuleSymbols> {
        let normalized = normalize_path(path);
        self.modules
            .values()
            .find(|module| normalize_path(&module.path) == normalized)
    }

    fn find_module_member(&self, module: &str, member: &str) -> Option<&SymbolDecl> {
        self.modules.get(module)?.declarations.iter().find(|decl| {
            decl.name == member
                && matches!(decl.kind, IndexedKind::Function | IndexedKind::RustFunction)
        })
    }
}

pub fn sibling_and_std_modules(path: &Path) -> Vec<String> {
    let mut names = STD_MODULES
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    if let Some(parent) = path.parent()
        && let Ok(entries) = std::fs::read_dir(parent)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("kiro")
                && let Some(stem) = path.file_stem().and_then(|stem| stem.to_str())
            {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

pub fn word_at(source: &str, position: Position) -> Option<String> {
    word_bounds_at(source, position).map(|(_, _, word)| word)
}

pub fn module_prefix_at(source: &str, position: Position) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let byte_idx = utf16_to_byte_idx(line, position.character as usize);
    let prefix = line.get(..byte_idx.min(line.len()))?;
    let before_dot = prefix.strip_suffix('.')?;
    let module = before_dot
        .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .next()?;
    (!module.is_empty()).then(|| module.to_string())
}

pub fn is_import_completion(source: &str, position: Position) -> bool {
    let Some(line) = source.lines().nth(position.line as usize) else {
        return false;
    };
    let byte_idx = utf16_to_byte_idx(line, position.character as usize);
    let prefix = line.get(..byte_idx.min(line.len())).unwrap_or(line);
    prefix.trim_start().starts_with("import ")
}

pub fn type_label(ty: &ast::KiroType) -> String {
    match ty {
        ast::KiroType::Num => "num".to_string(),
        ast::KiroType::Str => "str".to_string(),
        ast::KiroType::Bool => "bool".to_string(),
        ast::KiroType::Void => "void".to_string(),
        ast::KiroType::Adr(_, inner) => format!("adr {}", type_label(inner)),
        ast::KiroType::Pipe(_, inner) => format!("pipe {}", type_label(inner)),
        ast::KiroType::List(_, inner) => format!("list {}", type_label(inner)),
        ast::KiroType::Map(_, key, value) => {
            format!("map {} {}", type_label(key), type_label(value))
        }
        ast::KiroType::FnType(_, _, params, _, _, ret) => {
            let params = params.iter().map(type_label).collect::<Vec<_>>().join(", ");
            format!("fn({}) -> {}", params, type_label(ret))
        }
        ast::KiroType::Custom(name) => name.value.clone(),
    }
}

fn index_module(
    name: &str,
    path: PathBuf,
    source: String,
    program: &ast::Program,
) -> ModuleSymbols {
    let mut declarations = Vec::new();
    for stmt in &program.statements {
        collect_statement(name, &path, &source, stmt, None, &mut declarations);
    }
    ModuleSymbols {
        name: name.to_string(),
        path,
        source,
        declarations,
    }
}

fn index_module_line_first(name: &str, path: PathBuf, source: String) -> ModuleSymbols {
    let mut declarations = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if let Some(module) = trimmed.strip_prefix("import ").and_then(first_word) {
            declarations.push(decl(
                name,
                &path,
                &source,
                module,
                IndexedKind::Import,
                "import",
                None,
                Vec::new(),
                None,
            ));
        } else if let Some(fn_name) = function_name_from_line(trimmed) {
            declarations.push(decl(
                name,
                &path,
                &source,
                fn_name,
                IndexedKind::Function,
                if trimmed.starts_with("pure fn ") {
                    "pure fn"
                } else {
                    "fn"
                },
                Some(signature_from_line(trimmed)),
                Vec::new(),
                None,
            ));
        } else if let Some(fn_name) = rust_fn_name_from_line(trimmed) {
            declarations.push(decl(
                name,
                &path,
                &source,
                fn_name,
                IndexedKind::RustFunction,
                "rust fn",
                Some(signature_from_line(trimmed)),
                Vec::new(),
                None,
            ));
        } else if let Some(struct_name) = trimmed.strip_prefix("struct ").and_then(first_word) {
            declarations.push(decl(
                name,
                &path,
                &source,
                struct_name,
                IndexedKind::Struct,
                "struct",
                Some(format!("struct {}", struct_name)),
                Vec::new(),
                None,
            ));
        } else if let Some(error_name) = trimmed.strip_prefix("error ").and_then(first_word) {
            declarations.push(decl(
                name,
                &path,
                &source,
                error_name,
                IndexedKind::Error,
                "error",
                Some(format!("error {}", error_name)),
                Vec::new(),
                None,
            ));
        } else if let Some(var_name) = trimmed.strip_prefix("var ").and_then(first_word) {
            declarations.push(decl(
                name,
                &path,
                &source,
                var_name,
                IndexedKind::Var,
                "variable",
                None,
                Vec::new(),
                None,
            ));
        }
    }
    ModuleSymbols {
        name: name.to_string(),
        path,
        source,
        declarations,
    }
}

fn first_word(input: &str) -> Option<&str> {
    let word = input
        .trim_start()
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .next()?;
    (!word.is_empty()).then_some(word)
}

fn function_name_from_line(line: &str) -> Option<&str> {
    line.strip_prefix("fn ")
        .or_else(|| line.strip_prefix("pure fn "))
        .and_then(first_word)
}

fn rust_fn_name_from_line(line: &str) -> Option<&str> {
    line.strip_prefix("rust fn ").and_then(first_word)
}

fn signature_from_line(line: &str) -> String {
    line.split('{')
        .next()
        .unwrap_or(line)
        .trim_end()
        .to_string()
}

fn collect_statement(
    module: &str,
    path: &Path,
    source: &str,
    stmt: &ast::Statement,
    doc: Option<String>,
    declarations: &mut Vec<SymbolDecl>,
) {
    match stmt {
        ast::Statement::Import { module_name, .. } => declarations.push(decl(
            module,
            path,
            source,
            module_name,
            IndexedKind::Import,
            "import",
            None,
            Vec::new(),
            doc,
        )),
        ast::Statement::FunctionDef(def) => {
            push_function(module, path, source, def, doc, declarations)
        }
        ast::Statement::RustFnDecl(def) => {
            push_rust_fn(module, path, source, def, doc, declarations)
        }
        ast::Statement::StructDef(def) => declarations.push(decl(
            module,
            path,
            source,
            &def.name.value,
            IndexedKind::Struct,
            &format!("struct {}", def.name.value),
            Some(format!("struct {}", def.name.value)),
            Vec::new(),
            doc,
        )),
        ast::Statement::ErrorDef { name, .. } => declarations.push(decl(
            module,
            path,
            source,
            name,
            IndexedKind::Error,
            "error",
            Some(format!("error {}", name)),
            Vec::new(),
            doc,
        )),
        ast::Statement::VarDecl { ident, .. } => declarations.push(decl(
            module,
            path,
            source,
            ident,
            IndexedKind::Var,
            "variable",
            None,
            Vec::new(),
            doc,
        )),
        ast::Statement::Documented { doc, item } => {
            let doc = doc_text(doc);
            match item {
                ast::AnnotatableItem::FunctionDef(def) => {
                    push_function(module, path, source, def, doc, declarations)
                }
                ast::AnnotatableItem::RustFnDecl(def) => {
                    push_rust_fn(module, path, source, def, doc, declarations)
                }
                ast::AnnotatableItem::StructDef(def) => declarations.push(decl(
                    module,
                    path,
                    source,
                    &def.name.value,
                    IndexedKind::Struct,
                    &format!("struct {}", def.name.value),
                    Some(format!("struct {}", def.name.value)),
                    Vec::new(),
                    doc,
                )),
            }
        }
        _ => {}
    }
}

fn push_function(
    module: &str,
    path: &Path,
    source: &str,
    def: &ast::FunctionDef,
    doc: Option<String>,
    declarations: &mut Vec<SymbolDecl>,
) {
    let params = def
        .params
        .iter()
        .map(|param| format!("{}: {}", param.name, type_label(&param.command_type)))
        .collect::<Vec<_>>();
    let signature = function_signature(
        if def.pure_kw.is_some() {
            "pure fn"
        } else {
            "fn"
        },
        &def.name,
        &params,
        def.return_type.as_ref(),
        def.can_error.is_some(),
    );
    let detail = if def.pure_kw.is_some() {
        "pure fn"
    } else {
        "fn"
    };
    declarations.push(decl(
        module,
        path,
        source,
        &def.name,
        IndexedKind::Function,
        detail,
        Some(signature),
        params,
        doc,
    ));
}

fn push_rust_fn(
    module: &str,
    path: &Path,
    source: &str,
    def: &ast::RustFnDecl,
    doc: Option<String>,
    declarations: &mut Vec<SymbolDecl>,
) {
    let params = def
        .params
        .iter()
        .map(|param| format!("{}: {}", param.name, type_label(&param.command_type)))
        .collect::<Vec<_>>();
    let signature = function_signature(
        "rust fn",
        &def.name,
        &params,
        Some(&def.return_type),
        def.can_error.is_some(),
    );
    declarations.push(decl(
        module,
        path,
        source,
        &def.name,
        IndexedKind::RustFunction,
        "rust fn",
        Some(signature),
        params,
        doc,
    ));
}

fn decl(
    module: &str,
    path: &Path,
    source: &str,
    name: &str,
    kind: IndexedKind,
    detail: &str,
    signature: Option<String>,
    params: Vec<String>,
    doc: Option<String>,
) -> SymbolDecl {
    let range = declaration_range(source, name, &kind);
    SymbolDecl {
        name: name.to_string(),
        kind,
        module: module.to_string(),
        path: path.to_path_buf(),
        range,
        selection_range: range,
        detail: detail.to_string(),
        signature,
        params,
        doc,
    }
}

fn function_signature(
    prefix: &str,
    name: &str,
    params: &[String],
    return_type: Option<&ast::KiroType>,
    can_error: bool,
) -> String {
    let params = params.join(", ");
    let mut out = format!("{} {}({})", prefix, name, params);
    if let Some(return_type) = return_type {
        out.push_str(&format!(" -> {}", type_label(return_type)));
    }
    if can_error {
        out.push('!');
    }
    out
}

fn doc_text(docs: &[ast::DocComment]) -> Option<String> {
    let text = docs
        .iter()
        .map(|doc| doc.content.trim_start_matches("///").trim().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

fn hover_text(decl: &SymbolDecl) -> String {
    let mut text = decl.signature.clone().unwrap_or_else(|| match decl.kind {
        IndexedKind::Import => format!("module {}", decl.name),
        _ => decl.detail.clone(),
    });
    if let Some(doc) = &decl.doc
        && !doc.is_empty()
    {
        text.push_str("\n\n");
        text.push_str(doc);
    }
    text
}

fn declaration_range(source: &str, name: &str, kind: &IndexedKind) -> Range {
    let prefix = match kind {
        IndexedKind::Import => "import",
        IndexedKind::Function => "fn",
        IndexedKind::RustFunction => "rust fn",
        IndexedKind::Struct => "struct",
        IndexedKind::Error => "error",
        IndexedKind::Var => "var",
    };

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        let matches_kind = match kind {
            IndexedKind::Function => {
                trimmed.starts_with(&format!("fn {}", name))
                    || trimmed.starts_with(&format!("pure fn {}", name))
            }
            IndexedKind::RustFunction => trimmed.starts_with(&format!("rust fn {}", name)),
            _ => trimmed.starts_with(&format!("{} {}", prefix, name)),
        };
        if matches_kind && let Some(col) = line.find(name) {
            return range_for_line_span(line_idx, line, col, name.len());
        }
    }

    find_token_range(source, name).unwrap_or_default()
}

fn find_token_range(source: &str, token: &str) -> Option<Range> {
    for (line_idx, line) in source.lines().enumerate() {
        if let Some(col) = line.find(token) {
            return Some(range_for_line_span(line_idx, line, col, token.len()));
        }
    }
    None
}

fn range_for_line_span(line_idx: usize, line: &str, byte_col: usize, byte_len: usize) -> Range {
    let start = utf16_col(line, byte_col);
    let end = utf16_col(line, byte_col.saturating_add(byte_len).min(line.len()));
    Range {
        start: Position::new(line_idx as u32, start as u32),
        end: Position::new(line_idx as u32, end.max(start + 1) as u32),
    }
}

fn location_for(decl: &SymbolDecl) -> Option<Location> {
    let uri = uri_for_path(&decl.path)?;
    Some(Location::new(uri, decl.range))
}

fn module_location(module: &ModuleSymbols) -> Option<Location> {
    let uri = uri_for_path(&module.path)?;
    let range = module
        .declarations
        .first()
        .map(|decl| decl.range)
        .unwrap_or_default();
    Some(Location::new(uri, range))
}

fn uri_for_path(path: &Path) -> Option<Uri> {
    if !path.is_absolute() {
        return None;
    }
    let path = normalize_path(path);
    let url = Url::from_file_path(path).ok()?;
    Uri::from_str(url.as_str()).ok()
}

fn normalize_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn member_access_at(source: &str, position: Position) -> Option<(String, String)> {
    let line = source.lines().nth(position.line as usize)?;
    let (start, _, word) = word_bounds_at(source, position)?;
    if start > 0 && line.as_bytes().get(start - 1) == Some(&b'.') {
        let module_end = start - 1;
        let module_start = line[..module_end]
            .rfind(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .map(|idx| idx + 1)
            .unwrap_or(0);
        let module = line[module_start..module_end].to_string();
        if !module.is_empty() {
            return Some((module, word));
        }
    }
    None
}

fn word_bounds_at(source: &str, position: Position) -> Option<(usize, usize, String)> {
    let line = source.lines().nth(position.line as usize)?;
    let char_idx = utf16_to_byte_idx(line, position.character as usize);
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
    Some((start, end, line[start..end].to_string()))
}

struct CallContext {
    call_name: String,
    active_parameter: usize,
}

fn call_context_at(source: &str, position: Position) -> Option<CallContext> {
    let line = source.lines().nth(position.line as usize)?;
    let byte_idx = utf16_to_byte_idx(line, position.character as usize);
    let prefix = line.get(..byte_idx.min(line.len()))?;
    let paren_idx = prefix.rfind('(')?;
    let before_paren = prefix[..paren_idx].trim_end();
    let name_start = before_paren
        .rfind(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'))
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let call_name = before_paren[name_start..].to_string();
    if call_name.is_empty() {
        return None;
    }
    let active_parameter = prefix[paren_idx + 1..]
        .chars()
        .filter(|ch| *ch == ',')
        .count();
    Some(CallContext {
        call_name,
        active_parameter,
    })
}

fn utf16_to_byte_idx(line: &str, utf16_col: usize) -> usize {
    let mut units = 0;
    for (byte_idx, ch) in line.char_indices() {
        if units >= utf16_col {
            return byte_idx;
        }
        units += ch.len_utf16();
    }
    line.len()
}

fn utf16_col(line: &str, byte_col: usize) -> usize {
    line[..byte_col.min(line.len())]
        .chars()
        .map(char::len_utf16)
        .sum()
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
