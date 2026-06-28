use std::path::{Path, PathBuf};
use std::process::Command;

use crate::errors::{ErrorCode, ErrorPhase, KiroError};
use crate::project;

#[derive(Debug)]
pub struct TestSummary {
    pub passed: usize,
    pub failed: usize,
}

impl TestSummary {
    pub fn is_success(&self) -> bool {
        self.failed == 0
    }
}

pub fn run(paths: &[String]) -> Result<TestSummary, KiroError> {
    let cwd = std::env::current_dir().map_err(|e| {
        KiroError::new(
            ErrorCode::FileNotFound,
            ErrorPhase::Cli,
            format!("Failed to read current directory: {}", e),
        )
    })?;

    let run_root = project::find_project(&cwd)?
        .map(|project| project.root)
        .unwrap_or_else(|| cwd.clone());

    let files = discover_tests(paths, &run_root, &cwd)?;
    if files.is_empty() {
        println!("No tests found.");
        return Ok(TestSummary {
            passed: 0,
            failed: 0,
        });
    }

    let mut passed = 0usize;
    let mut failed = 0usize;
    for file in files {
        let display = display_path(&file, &run_root);
        let output = run_one_test(&file, &run_root)?;
        if output.status.success() {
            println!("PASS {}", display);
            passed += 1;
        } else {
            println!("FAIL {}", display);
            print_captured_output("stdout", &output.stdout);
            print_captured_output("stderr", &output.stderr);
            failed += 1;
        }
    }

    if failed == 0 {
        println!("test result: ok. {} passed; 0 failed.", passed);
    } else {
        println!("test result: FAILED. {} passed; {} failed.", passed, failed);
    }

    Ok(TestSummary { passed, failed })
}

fn discover_tests(
    paths: &[String],
    run_root: &Path,
    cwd: &Path,
) -> Result<Vec<PathBuf>, KiroError> {
    let mut files = Vec::new();
    if paths.is_empty() {
        collect_test_files(run_root, true, &mut files)?;
    } else {
        for path in paths {
            let path = cwd.join(path);
            collect_test_files(&path, false, &mut files)?;
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_test_files(
    path: &Path,
    require_test_suffix: bool,
    files: &mut Vec<PathBuf>,
) -> Result<(), KiroError> {
    if !path.exists() {
        return Err(KiroError::file_not_found(&path.display().to_string()));
    }

    if path.is_file() {
        if path.extension().is_some_and(|ext| ext == "kiro")
            && (!require_test_suffix || is_test_file(path))
        {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }

    if should_skip_dir(path) {
        return Ok(());
    }

    let entries = std::fs::read_dir(path).map_err(|e| {
        KiroError::new(
            ErrorCode::FileNotFound,
            ErrorPhase::Cli,
            format!("Failed to read '{}': {}", path.display(), e),
        )
        .with_file(path.display().to_string())
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            KiroError::new(
                ErrorCode::FileNotFound,
                ErrorPhase::Cli,
                format!("Failed to read directory entry: {}", e),
            )
        })?;
        collect_test_files(&entry.path(), true, files)?;
    }
    Ok(())
}

fn is_test_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("_test.kiro"))
}

fn should_skip_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name == ".git" || name == "target" || name == "kiro_build_cache" || name.starts_with('.')
}

fn run_one_test(file: &Path, run_root: &Path) -> Result<std::process::Output, KiroError> {
    let binary = std::env::current_exe().map_err(|e| {
        KiroError::new(
            ErrorCode::FileNotFound,
            ErrorPhase::Cli,
            format!("Failed to find current executable: {}", e),
        )
    })?;
    let arg = display_path(file, run_root);
    Command::new(binary)
        .args(["run", &arg, "--no-interpret"])
        .current_dir(run_root)
        .output()
        .map_err(|e| {
            KiroError::new(
                ErrorCode::BuildGraphFailed,
                ErrorPhase::Cli,
                format!("Failed to run test '{}': {}", arg, e),
            )
            .with_file(arg)
        })
}

fn display_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn print_captured_output(label: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    println!("--- {} ---", label);
    print!("{}", String::from_utf8_lossy(bytes));
    if !bytes.ends_with(b"\n") {
        println!();
    }
}
