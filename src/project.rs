use std::path::{Path, PathBuf};

use crate::errors::{ErrorCode, ErrorPhase, KiroError};
use crate::is_reserved_std_module_name;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoDependency {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct KiroProject {
    pub root: PathBuf,
    pub manifest_path: PathBuf,
    pub package_name: String,
    pub entry: PathBuf,
    pub dependencies: Vec<CargoDependency>,
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

    let dependencies = parse_dependencies(&doc, &manifest_path)?;

    Ok(KiroProject {
        root,
        manifest_path,
        package_name,
        entry: PathBuf::from(entry),
        dependencies,
    })
}

fn parse_dependencies(
    doc: &toml_edit::DocumentMut,
    manifest_path: &Path,
) -> Result<Vec<CargoDependency>, KiroError> {
    let Some(deps) = doc.get("dependencies") else {
        return Ok(Vec::new());
    };

    let table = deps.as_table_like().ok_or_else(|| {
        manifest_error(
            manifest_path,
            "kiro.toml [dependencies] must be a table of string versions.",
        )
    })?;

    let mut parsed = Vec::new();
    for (name, item) in table.iter() {
        if !is_valid_cargo_dependency_name(name) {
            return Err(manifest_error(
                manifest_path,
                format!("Invalid dependency name '{}'.", name),
            ));
        }
        if is_reserved_std_module_name(name) {
            return Err(manifest_error(
                manifest_path,
                format!(
                    "Dependency '{}' conflicts with a reserved Kiro std module name.",
                    name
                ),
            ));
        }

        if item.as_table_like().is_some() {
            return Err(manifest_error(
                manifest_path,
                format!(
                    "Dependency '{}' uses a table spec, but V1 only supports `{} = \"version\"`.",
                    name, name
                ),
            )
            .with_help("use a simple string version such as `image = \"0.25\"`"));
        }

        let Some(version) = item.as_str() else {
            return Err(manifest_error(
                manifest_path,
                format!(
                    "Dependency '{}' must use a string version such as `{} = \"1\"`.",
                    name, name
                ),
            ));
        };
        if version.trim().is_empty() {
            return Err(manifest_error(
                manifest_path,
                format!("Dependency '{}' version must not be empty.", name),
            ));
        }

        parsed.push(CargoDependency {
            name: name.to_string(),
            version: version.to_string(),
        });
    }

    parsed.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(parsed)
}

fn manifest_error(path: &Path, message: impl Into<String>) -> KiroError {
    KiroError::new(ErrorCode::ParseFailed, ErrorPhase::Cli, message.into())
        .with_file(path.display().to_string())
}

pub fn is_valid_cargo_dependency_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}
