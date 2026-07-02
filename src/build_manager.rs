use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct BuildRequirements {
    pub uses_std_fs: bool,
    pub uses_std_time: bool,
    pub uses_std_net: bool,
    pub uses_pipes: bool,
    pub uses_anyhow: bool,
    pub skipped_module_imports: HashSet<String>,
}

impl BuildRequirements {
    pub fn record_module(&mut self, module_name: &str) {
        match module_name {
            "std_fs" => self.uses_std_fs = true,
            "std_time" => self.uses_std_time = true,
            "std_net" => self.uses_std_net = true,
            _ => {}
        }
    }

    pub fn record_pipes(&mut self, uses_pipes: bool) {
        self.uses_pipes |= uses_pipes;
    }

    pub fn record_anyhow(&mut self, uses_anyhow: bool) {
        self.uses_anyhow |= uses_anyhow;
    }

    pub fn skip_module_import(&mut self, module_name: impl Into<String>) {
        self.skipped_module_imports.insert(module_name.into());
    }

    pub fn skips_module_import(&self, module_name: &str) -> bool {
        self.skipped_module_imports.contains(module_name)
    }
}

pub struct BuildManager {
    build_dir: String,
}
impl BuildManager {
    const HEADER_PREAMBLE: &'static str = "//! Kiro Header - Generated glue code for rust fn\n\nuse kiro_runtime::{HostResult, KiroError, RuntimeVal};\n\n";

    pub fn new(build_dir: &str) -> Self {
        Self {
            build_dir: build_dir.to_string(),
        }
    }

    /// Sets up the folder structure and initializes generated source files.
    pub fn init(&self) -> Result<(), String> {
        let src_dir = format!("{}/src", self.build_dir);

        // 1. Create directories
        if !Path::new(&src_dir).exists() {
            fs::create_dir_all(&src_dir).map_err(|e| e.to_string())?;
            println!("📁 Initialized build directory: {}", self.build_dir);
        }

        Ok(())
    }

    pub fn header_preamble() -> &'static str {
        Self::HEADER_PREAMBLE
    }

    pub fn save_file(&self, name_without_ext: &str, code: String) -> Result<(), String> {
        let file_path = format!("{}/src/{}.rs", self.build_dir, name_without_ext);
        if write_if_changed(&file_path, &code)? {
            println!("💾 Code saved to {}", file_path);
        }
        Ok(())
    }

    pub fn save_header(&self, content: &str) -> Result<(), String> {
        let header_path = format!("{}/src/header.rs", self.build_dir);
        write_if_changed(&header_path, content)?;
        Ok(())
    }

    pub fn build(&self, verbose: bool) -> Result<std::path::PathBuf, String> {
        if verbose {
            println!("🚀 Compiling...\n");
        }

        let output = Command::new("cargo")
            .arg("build")
            .arg("--quiet") // Less noise
            .env("CARGO_TARGET_DIR", "target")
            .current_dir(&self.build_dir)
            .output()
            .map_err(|e| format!("Failed to execute cargo: {}", e))?;

        if verbose && !output.stdout.is_empty() {
            println!("{}", String::from_utf8_lossy(&output.stdout));
        }

        // Show stderr if verbose OR if compilation failed
        if (!output.status.success() || verbose) && !output.stderr.is_empty() {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        }

        if output.status.success() {
            let exe_path = Path::new(&self.build_dir)
                .join("target")
                .join("debug")
                .join("kiro_script");
            Ok(exe_path)
        } else {
            Err("Compilation failed.".to_string())
        }
    }

    pub fn write_cargo_toml(&self, requirements: &BuildRequirements) -> Result<(), String> {
        let mut tokio_features = vec!["macros", "rt-multi-thread"];
        if requirements.uses_std_fs {
            tokio_features.push("fs");
        }
        if requirements.uses_std_time {
            tokio_features.push("time");
        }

        let tokio_features = tokio_features
            .iter()
            .map(|feature| format!(r#""{}""#, feature))
            .collect::<Vec<_>>()
            .join(", ");

        let mut dependency_lines = vec![
            format!(
                r#"tokio = {{ version = "1.49.0", features = [{}] }}"#,
                tokio_features
            ),
            r#"kiro_runtime = { path = "../kiro_runtime" }"#.to_string(),
        ];

        if requirements.uses_anyhow {
            dependency_lines.push(r#"anyhow = "1""#.to_string());
        }
        if requirements.uses_pipes {
            dependency_lines.push(r#"async-channel = "2.5.0""#.to_string());
        }
        if requirements.uses_std_net {
            dependency_lines.push(
                r#"reqwest = { version = "0.13.1", features = ["gzip", "json"] }"#.to_string(),
            );
        }

        let content = format!(
            r#"
[package]
name = "kiro_script"
version = "0.1.0"
edition = "2021"

[dependencies]
{}
"#,
            dependency_lines.join("\n")
        );
        write_if_changed(format!("{}/Cargo.toml", self.build_dir), &content).map(|_| ())
    }
}

fn write_if_changed(path: impl AsRef<Path>, content: &str) -> Result<bool, String> {
    let path = path.as_ref();
    if let Ok(existing) = fs::read_to_string(path)
        && existing == content
    {
        return Ok(false);
    }
    fs::write(path, content).map_err(|e| e.to_string())?;
    Ok(true)
}
