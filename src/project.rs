use std::path::{Path, PathBuf};

use crate::errors::{ErrorCode, ErrorPhase, KiroError};

#[derive(Debug, Clone)]
pub struct KiroProject {
    pub root: PathBuf,
    pub manifest_path: PathBuf,
    pub package_name: String,
    pub entry: PathBuf,
}

impl KiroProject {
    pub fn entry_path(&self) -> PathBuf {
        self.root.join(&self.entry)
    }
}

pub fn find_project(start: impl AsRef<Path>) -> Result<Option<KiroProject>, KiroError> {
    let mut dir = start.as_ref().to_path_buf();
    if dir.is_file() {
        dir = dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
    }

    loop {
        let manifest_path = dir.join("kiro.toml");
        if manifest_path.exists() {
            return Ok(Some(load_project(manifest_path)?));
        }

        if !dir.pop() {
            return Ok(None);
        }
    }
}

pub fn no_input_error() -> KiroError {
    KiroError::new(
        ErrorCode::FileNotFound,
        ErrorPhase::Cli,
        "No input file and no kiro.toml found.",
    )
    .with_help("run `kiro main.kiro` or create a project with `kiro create app`")
}

fn load_project(manifest_path: PathBuf) -> Result<KiroProject, KiroError> {
    let source = std::fs::read_to_string(&manifest_path).map_err(|e| {
        KiroError::new(
            ErrorCode::FileNotFound,
            ErrorPhase::Cli,
            format!("Failed to read '{}': {}", manifest_path.display(), e),
        )
        .with_file(manifest_path.display().to_string())
    })?;
    let doc = source.parse::<toml_edit::DocumentMut>().map_err(|e| {
        KiroError::new(
            ErrorCode::ParseFailed,
            ErrorPhase::Cli,
            format!("Invalid kiro.toml: {}", e),
        )
        .with_file(manifest_path.display().to_string())
    })?;

    let package = doc
        .get("package")
        .and_then(|item| item.as_table())
        .ok_or_else(|| {
            KiroError::new(
                ErrorCode::ParseFailed,
                ErrorPhase::Cli,
                "kiro.toml is missing [package].",
            )
            .with_file(manifest_path.display().to_string())
        })?;

    let package_name = package
        .get("name")
        .and_then(|item| item.as_str())
        .unwrap_or("kiro_project")
        .to_string();

    let entry = package
        .get("entry")
        .and_then(|item| item.as_str())
        .ok_or_else(|| {
            KiroError::new(
                ErrorCode::ParseFailed,
                ErrorPhase::Cli,
                "kiro.toml is missing [package].entry.",
            )
            .with_file(manifest_path.display().to_string())
            .with_help("add `entry = \"main.kiro\"` under [package]")
        })?;

    if entry.trim().is_empty() {
        return Err(KiroError::new(
            ErrorCode::ParseFailed,
            ErrorPhase::Cli,
            "kiro.toml [package].entry must not be empty.",
        )
        .with_file(manifest_path.display().to_string()));
    }

    let root = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    Ok(KiroProject {
        root,
        manifest_path,
        package_name,
        entry: PathBuf::from(entry),
    })
}
