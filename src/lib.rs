pub mod analysis;
pub mod build_manager;
pub mod compiler;
pub mod engine;
pub mod errors;
pub mod formatter;
pub mod grammar;
pub mod interpreter;
#[cfg(feature = "lsp")]
pub mod lsp;
#[cfg(feature = "lsp")]
pub mod lsp_symbols;
pub mod project;
pub mod test_runner;

pub struct StdAssets;

pub struct StdAsset {
    pub data: &'static [u8],
}

pub const STD_MODULE_ALIASES: &[(&str, &str)] = &[
    ("env", "std_env"),
    ("fs", "std_fs"),
    ("io", "std_io"),
    ("net", "std_net"),
    ("time", "std_time"),
];

pub fn canonical_std_module_name(name: &str) -> Option<&'static str> {
    match name {
        "std_env" | "env" => Some("std_env"),
        "std_fs" | "fs" => Some("std_fs"),
        "std_io" | "io" => Some("std_io"),
        "std_net" | "net" => Some("std_net"),
        "std_time" | "time" => Some("std_time"),
        _ => None,
    }
}

pub fn std_module_suffix(name: &str) -> Option<&'static str> {
    canonical_std_module_name(name).map(|canonical| canonical.trim_start_matches("std_"))
}

pub fn is_reserved_std_module_name(name: &str) -> bool {
    name.starts_with("std_") || canonical_std_module_name(name).is_some()
}

pub fn std_asset_path(name: &str, file: &str) -> Option<String> {
    let canonical = canonical_std_module_name(name)?;
    let suffix = canonical.trim_start_matches("std_");
    Some(format!("{}/{}", suffix, file))
}

pub fn is_std_io_module_name(name: &str) -> bool {
    canonical_std_module_name(name) == Some("std_io")
}

pub fn is_std_io_display_function(name: &str) -> bool {
    matches!(name, "print" | "write" | "eprint" | "eprintline")
}

impl StdAssets {
    pub fn get(path: &str) -> Option<StdAsset> {
        let data = match path {
            "env/header.rs" => include_bytes!("kiro_std/env/header.rs").as_slice(),
            "env/std_env.kiro" => include_bytes!("kiro_std/env/std_env.kiro").as_slice(),
            "fs/header.rs" => include_bytes!("kiro_std/fs/header.rs").as_slice(),
            "fs/std_fs.kiro" => include_bytes!("kiro_std/fs/std_fs.kiro").as_slice(),
            "io/header.rs" => include_bytes!("kiro_std/io/header.rs").as_slice(),
            "io/std_io.kiro" => include_bytes!("kiro_std/io/std_io.kiro").as_slice(),
            "net/header.rs" => include_bytes!("kiro_std/net/header.rs").as_slice(),
            "net/std_net.kiro" => include_bytes!("kiro_std/net/std_net.kiro").as_slice(),
            "time/header.rs" => include_bytes!("kiro_std/time/header.rs").as_slice(),
            "time/std_time.kiro" => include_bytes!("kiro_std/time/std_time.kiro").as_slice(),
            _ => return None,
        };
        Some(StdAsset { data })
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemovedPrintStatement {
    pub line: usize,
    pub column: usize,
}

pub fn removed_print_statement(source: &str) -> Option<RemovedPrintStatement> {
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let leading_ws = line.len().saturating_sub(trimmed.len());
        if trimmed == "print" {
            return Some(RemovedPrintStatement {
                line: idx + 1,
                column: leading_ws + 1,
            });
        }
        if let Some(rest) = trimmed.strip_prefix("print")
            && rest
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_whitespace())
        {
            return Some(RemovedPrintStatement {
                line: idx + 1,
                column: leading_ws + 1,
            });
        }
    }
    None
}
