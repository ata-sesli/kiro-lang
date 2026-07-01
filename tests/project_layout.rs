use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static KIRO_BUILD_LOCK: Mutex<()> = Mutex::new(());

fn temp_project(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "kiro_project_layout_{}_{}_{}",
        name,
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&dir).expect("temp project should be created");
    dir
}

fn run_kiro(args: &[&str], current_dir: &Path) -> std::process::Output {
    let _guard = KIRO_BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
        .args(args)
        .current_dir(current_dir)
        .output()
        .expect("kiro-lang command should run")
}

fn write_manifest(dir: &Path, entry: &str) {
    fs::write(
        dir.join("kiro.toml"),
        format!(
            r#"[package]
name = "demo"
entry = "{}"

[dependencies]
"#,
            entry
        ),
    )
    .expect("manifest should be written");
}

fn link_runtime(project_dir: &Path) {
    let runtime_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("kiro_runtime");
    let runtime_dst = project_dir.join("kiro_runtime");
    #[cfg(unix)]
    symlink(&runtime_src, &runtime_dst).expect("kiro_runtime symlink should be created");
    #[cfg(not(unix))]
    {
        fs::create_dir_all(runtime_dst.join("src"))
            .expect("kiro_runtime src dir should be created");
        fs::copy(
            runtime_src.join("Cargo.toml"),
            runtime_dst.join("Cargo.toml"),
        )
        .expect("kiro_runtime Cargo.toml should be copied");
        fs::copy(
            runtime_src.join("src/lib.rs"),
            runtime_dst.join("src/lib.rs"),
        )
        .expect("kiro_runtime lib.rs should be copied");
    }
}

#[test]
fn no_file_run_uses_manifest_entry() {
    let dir = temp_project("run_entry");
    link_runtime(&dir);
    write_manifest(&dir, "main.kiro");
    fs::write(dir.join("main.kiro"), "print \"manifest run\"\n").expect("entry should be written");

    let output = run_kiro(&["run"], &dir);

    assert!(
        output.status.success(),
        "manifest run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("manifest run"),
        "stdout should include entry output:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn no_file_build_and_check_use_manifest_entry() {
    let dir = temp_project("build_check_entry");
    link_runtime(&dir);
    write_manifest(&dir, "main.kiro");
    fs::write(dir.join("main.kiro"), "print \"manifest command\"\n")
        .expect("entry should be written");

    let build = run_kiro(&["build"], &dir);
    assert!(
        build.status.success(),
        "manifest build should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let check = run_kiro(&["check"], &dir);
    assert!(
        check.status.success(),
        "manifest check should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(
        String::from_utf8_lossy(&check.stdout).contains("OK"),
        "check should statically validate manifest entry:\n{}",
        String::from_utf8_lossy(&check.stdout)
    );
}

#[test]
fn bare_kiro_uses_manifest_entry_from_subdirectory() {
    let dir = temp_project("walk_up");
    link_runtime(&dir);
    let nested = dir.join("tasks/nested");
    fs::create_dir_all(&nested).expect("nested dir should be created");
    write_manifest(&dir, "main.kiro");
    fs::write(dir.join("main.kiro"), "print \"walked up\"\n").expect("entry should be written");

    let output = run_kiro(&[], &nested);

    assert!(
        output.status.success(),
        "bare kiro should find parent manifest\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("walked up"),
        "stdout should include parent entry output:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn explicit_file_overrides_manifest_entry() {
    let dir = temp_project("explicit_override");
    link_runtime(&dir);
    write_manifest(&dir, "main.kiro");
    fs::write(dir.join("main.kiro"), "print \"manifest\"\n").expect("entry should be written");
    fs::write(dir.join("other.kiro"), "print \"explicit\"\n")
        .expect("other file should be written");

    let output = run_kiro(&["run", "other.kiro"], &dir);

    assert!(
        output.status.success(),
        "explicit file should run\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("explicit"),
        "unexpected stdout:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("manifest"),
        "manifest entry should not run:\n{}",
        stdout
    );
}

#[test]
fn missing_manifest_entry_is_a_kiro_diagnostic() {
    let dir = temp_project("missing_entry");
    fs::write(
        dir.join("kiro.toml"),
        r#"[package]
name = "demo"

[dependencies]
"#,
    )
    .expect("manifest should be written");

    let output = run_kiro(&["build"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "missing entry should fail");
    assert!(
        stderr.contains("[KIRO1003:cli] kiro.toml is missing [package].entry."),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn missing_manifest_entry_file_is_a_kiro_diagnostic() {
    let dir = temp_project("missing_entry_file");
    write_manifest(&dir, "missing.kiro");

    let output = run_kiro(&["build"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "missing entry file should fail");
    assert!(stderr.contains("File '"), "unexpected stderr:\n{}", stderr);
    assert!(
        stderr.contains("missing.kiro"),
        "diagnostic should mention missing entry file:\n{}",
        stderr
    );
}

#[test]
fn no_file_without_manifest_prints_helpful_message() {
    let dir = temp_project("no_manifest");

    let output = run_kiro(&["run"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "no input should fail");
    assert!(
        stderr.contains("No input file and no kiro.toml found."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("help: run `kiro main.kiro` or create a project with `kiro create app`"),
        "diagnostic should include help:\n{}",
        stderr
    );
}

#[test]
fn local_modules_remain_sibling_flat_modules() {
    let dir = temp_project("flat_modules");
    link_runtime(&dir);
    write_manifest(&dir, "main.kiro");
    fs::write(
        dir.join("main.kiro"),
        r#"
import math

print math.add(2, 3)
"#,
    )
    .expect("main should be written");
    fs::write(
        dir.join("math.kiro"),
        r#"
pure fn add(a: num, b: num) -> num {
    return a + b
}
"#,
    )
    .expect("module should be written");

    let output = run_kiro(&["run"], &dir);

    assert!(
        output.status.success(),
        "flat module import should run\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("5"),
        "stdout should contain module result:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}
