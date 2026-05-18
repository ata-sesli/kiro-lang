pub mod build_manager;
pub mod compiler;
pub mod engine;
pub mod errors;
pub mod formatter;
pub mod grammar;
pub mod interpreter;

#[derive(rust_embed::RustEmbed)]
#[folder = "src/kiro_std/"]
pub struct StdAssets;

#[derive(rust_embed::RustEmbed)]
#[folder = "kiro_runtime/"]
pub struct RuntimeAssets;

pub fn unsupported_let_line(source: &str) -> Option<usize> {
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if trimmed.split_whitespace().next() == Some("let") {
            return Some(idx + 1);
        }
    }
    None
}
