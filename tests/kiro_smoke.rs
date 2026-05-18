use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static KIRO_BUILD_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug)]
struct SmokeCase {
    name: String,
    path: PathBuf,
    command: String,
    expect: String,
    stdout_contains: Vec<String>,
    stderr_contains: Vec<String>,
}

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/smoke/manifest.toml")
}

fn parse_string(value: &toml_edit::Value) -> String {
    value
        .as_str()
        .expect("manifest field should be a string")
        .to_string()
}

fn parse_string_array(item: Option<&toml_edit::Item>) -> Vec<String> {
    item.and_then(|i| i.as_array())
        .map(|arr| {
            arr.iter()
                .map(|value| {
                    value
                        .as_str()
                        .expect("manifest array entry should be a string")
                        .to_string()
                })
                .collect()
        })
        .unwrap_or_default()
}

fn smoke_cases() -> Vec<SmokeCase> {
    let manifest = fs::read_to_string(manifest_path()).expect("smoke manifest should exist");
    let doc = manifest
        .parse::<toml_edit::DocumentMut>()
        .expect("smoke manifest should parse");
    let cases = doc["case"]
        .as_array_of_tables()
        .expect("smoke manifest should contain [[case]] entries");

    cases
        .iter()
        .map(|case| SmokeCase {
            name: parse_string(
                case["name"]
                    .as_value()
                    .expect("smoke case should have a name"),
            ),
            path: PathBuf::from(parse_string(
                case["path"]
                    .as_value()
                    .expect("smoke case should have a path"),
            )),
            command: parse_string(
                case["command"]
                    .as_value()
                    .expect("smoke case should have a command"),
            ),
            expect: parse_string(
                case["expect"]
                    .as_value()
                    .expect("smoke case should have an expectation"),
            ),
            stdout_contains: parse_string_array(case.get("stdout_contains")),
            stderr_contains: parse_string_array(case.get("stderr_contains")),
        })
        .collect()
}

fn temp_project(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "kiro_smoke_{}_{}_{}",
        name,
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&dir).expect("temp project should be created");
    dir
}

fn copy_case_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("case temp dir should be created");
    for entry in fs::read_dir(src).expect("case dir should be readable") {
        let entry = entry.expect("case entry should be readable");
        let file_type = entry
            .file_type()
            .expect("case entry type should be readable");
        if file_type.is_file() {
            fs::copy(entry.path(), dst.join(entry.file_name()))
                .expect("case file should be copied");
        }
    }
}

fn link_runtime(repo_root: &Path, case_dir: &Path) {
    let runtime_src = repo_root.join("kiro_runtime");
    let runtime_dst = case_dir.join("kiro_runtime");
    #[cfg(unix)]
    symlink(&runtime_src, &runtime_dst).expect("kiro_runtime symlink should be created");
    #[cfg(not(unix))]
    {
        fs::create_dir_all(&runtime_dst).expect("kiro_runtime dir should be created");
        fs::copy(
            runtime_src.join("Cargo.toml"),
            runtime_dst.join("Cargo.toml"),
        )
        .expect("kiro_runtime Cargo.toml should be copied");
        fs::create_dir_all(runtime_dst.join("src"))
            .expect("kiro_runtime src dir should be created");
        fs::copy(
            runtime_src.join("src/lib.rs"),
            runtime_dst.join("src/lib.rs"),
        )
        .expect("kiro_runtime lib.rs should be copied");
    }
}

fn run_kiro(case: &SmokeCase, case_dir: &Path) -> std::process::Output {
    let _guard = KIRO_BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let binary = env!("CARGO_BIN_EXE_kiro-lang");
    let path = case_dir.join(
        case.path
            .file_name()
            .expect("case path should have a filename"),
    );

    let mut command = Command::new(binary);
    match case.command.as_str() {
        "run" => {
            command.args(["run", path.to_str().unwrap()]);
        }
        "check" => {
            command.args(["check", path.to_str().unwrap()]);
        }
        "build" => {
            command.args(["build", path.to_str().unwrap()]);
        }
        other => panic!("unknown smoke command '{}'", other),
    }

    command
        .current_dir(case_dir)
        .output()
        .expect("kiro-lang command should run")
}

#[test]
fn kiro_smoke_manifest_cases_behave_as_expected() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    for case in smoke_cases() {
        let source_path = root.join("tests/smoke").join(&case.path);
        let source_dir = source_path
            .parent()
            .expect("case source path should have a parent");
        let case_dir = temp_project(&case.name);
        copy_case_dir(source_dir, &case_dir);
        link_runtime(&root, &case_dir);

        let output = run_kiro(&case, &case_dir);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        match case.expect.as_str() {
            "pass" => assert!(
                output.status.success(),
                "smoke case '{}' should pass\nstdout:\n{}\nstderr:\n{}",
                case.name,
                stdout,
                stderr
            ),
            "fail" => assert!(
                !output.status.success(),
                "smoke case '{}' should fail\nstdout:\n{}\nstderr:\n{}",
                case.name,
                stdout,
                stderr
            ),
            other => panic!("unknown smoke expectation '{}'", other),
        }

        for expected in &case.stdout_contains {
            assert!(
                stdout.contains(expected),
                "smoke case '{}' stdout should contain '{}'\nstdout:\n{}\nstderr:\n{}",
                case.name,
                expected,
                stdout,
                stderr
            );
        }

        for expected in &case.stderr_contains {
            assert!(
                stderr.contains(expected),
                "smoke case '{}' stderr should contain '{}'\nstdout:\n{}\nstderr:\n{}",
                case.name,
                expected,
                stdout,
                stderr
            );
        }
    }
}
