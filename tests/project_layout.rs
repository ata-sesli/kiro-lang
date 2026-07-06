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

fn write_add_fixture_crate(project_dir: &Path) {
    let crate_dir = project_dir.join("fixture_crate");
    fs::create_dir_all(crate_dir.join("src")).expect("fixture src should be created");
    fs::write(
        crate_dir.join("Cargo.toml"),
        r#"[package]
name = "kiro_add_fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("fixture manifest should be written");
    fs::write(
        crate_dir.join("src/lib.rs"),
        r#"pub fn add(a: f64, b: f64) -> f64 {
    a + b
}
"#,
    )
    .expect("fixture lib should be written");
    fs::write(
        project_dir.join("Cargo.toml"),
        r#"[package]
name = "demo_add_fixture"
version = "0.1.0"
edition = "2021"

[dependencies]
kiro_add_fixture = { path = "fixture_crate" }
"#,
    )
    .expect("metadata manifest should be written");
    fs::create_dir_all(project_dir.join("src")).expect("project src should be created");
    fs::write(project_dir.join("src/lib.rs"), "").expect("project lib should be written");
}

#[test]
fn no_file_run_uses_manifest_entry() {
    let dir = temp_project("run_entry");
    link_runtime(&dir);
    write_manifest(&dir, "main.kiro");
    fs::write(
        dir.join("main.kiro"),
        "import io\n\nio.print(\"manifest run\")\n",
    )
    .expect("entry should be written");

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
    fs::write(
        dir.join("main.kiro"),
        "import io\n\nio.print(\"manifest command\")\n",
    )
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
    fs::write(
        dir.join("main.kiro"),
        "import io\n\nio.print(\"walked up\")\n",
    )
    .expect("entry should be written");

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
    fs::write(
        dir.join("main.kiro"),
        "import io\n\nio.print(\"manifest\")\n",
    )
    .expect("entry should be written");
    fs::write(
        dir.join("other.kiro"),
        "import io\n\nio.print(\"explicit\")\n",
    )
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
fn table_dependency_specs_are_rejected_in_v1() {
    let dir = temp_project("table_dep");
    fs::write(
        dir.join("kiro.toml"),
        r#"[package]
name = "demo"
entry = "main.kiro"

[dependencies.reqwest]
version = "0.12"
"#,
    )
    .expect("manifest should be written");
    fs::write(dir.join("main.kiro"), "check true\n").expect("entry should be written");

    let output = run_kiro(&["check"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "table dependency should fail");
    assert!(
        stderr.contains("table spec") && stderr.contains("V1 only supports"),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn non_string_dependency_specs_are_rejected() {
    let dir = temp_project("non_string_dep");
    fs::write(
        dir.join("kiro.toml"),
        r#"[package]
name = "demo"
entry = "main.kiro"

[dependencies]
image = 25
"#,
    )
    .expect("manifest should be written");
    fs::write(dir.join("main.kiro"), "check true\n").expect("entry should be written");

    let output = run_kiro(&["check"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "non-string dependency should fail"
    );
    assert!(
        stderr.contains("must use a string version"),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn empty_dependency_versions_are_rejected() {
    let dir = temp_project("empty_dep_version");
    fs::write(
        dir.join("kiro.toml"),
        r#"[package]
name = "demo"
entry = "main.kiro"

[dependencies]
image = ""
"#,
    )
    .expect("manifest should be written");
    fs::write(dir.join("main.kiro"), "check true\n").expect("entry should be written");

    let output = run_kiro(&["check"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "empty dependency version should fail"
    );
    assert!(
        stderr.contains("version must not be empty"),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn invalid_dependency_names_are_rejected_before_cargo() {
    let dir = temp_project("invalid_dep_name");
    fs::write(
        dir.join("kiro.toml"),
        r#"[package]
name = "demo"
entry = "main.kiro"

[dependencies]
"bad name" = "1"
"#,
    )
    .expect("manifest should be written");
    fs::write(dir.join("main.kiro"), "check true\n").expect("entry should be written");

    let output = run_kiro(&["check"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "invalid dependency name should fail"
    );
    assert!(
        stderr.contains("Invalid dependency name 'bad name'"),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E") && !stderr.contains(".kiro/build"),
        "invalid manifest dependency should fail before Cargo/build output:\n{}",
        stderr
    );
}

#[test]
fn std_module_dependency_names_are_rejected() {
    let dir = temp_project("std_name_dep");
    fs::write(
        dir.join("kiro.toml"),
        r#"[package]
name = "demo"
entry = "main.kiro"

[dependencies]
io = "1"
"#,
    )
    .expect("manifest should be written");
    fs::write(dir.join("main.kiro"), "check true\n").expect("entry should be written");

    let output = run_kiro(&["check"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "reserved dependency should fail");
    assert!(
        stderr.contains("conflicts with a reserved Kiro std module name"),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn kiro_add_records_dependency_and_generates_host_module() {
    let dir = temp_project("add_remove");
    write_manifest(&dir, "main.kiro");
    write_add_fixture_crate(&dir);

    let add_versioned = run_kiro(&["add", "kiro_add_fixture@0.1.0"], &dir);
    assert!(
        add_versioned.status.success(),
        "kiro add kiro_add_fixture@0.1.0 should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&add_versioned.stdout),
        String::from_utf8_lossy(&add_versioned.stderr)
    );

    let manifest = fs::read_to_string(dir.join("kiro.toml")).expect("manifest should be readable");
    assert!(
        manifest.contains(r#"kiro_add_fixture = "0.1.0""#),
        "add should write the Cargo dependency spec:\n{}",
        manifest
    );
    assert!(
        dir.join("kiro_add_fixture.kiro").exists() && dir.join("kiro_add_fixture.rs").exists(),
        "kiro add should generate a host module pair"
    );
    let generated =
        fs::read_to_string(dir.join("kiro_add_fixture.kiro")).expect("generated module readable");
    assert!(
        generated.contains("rust fn add(a: num, b: num) -> num"),
        "generated bindings should include supported crate functions:\n{}",
        generated
    );

    let remove = run_kiro(&["remove", "kiro_add_fixture"], &dir);
    assert!(
        remove.status.success(),
        "kiro remove kiro_add_fixture should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&remove.stdout),
        String::from_utf8_lossy(&remove.stderr)
    );
    let manifest = fs::read_to_string(dir.join("kiro.toml")).expect("manifest should be readable");
    assert!(
        !manifest.contains("kiro_add_fixture"),
        "remove should delete only the named dependency:\n{}",
        manifest
    );
}

#[test]
fn kiro_add_without_version_records_resolved_package_version() {
    let dir = temp_project("add_latest");
    write_manifest(&dir, "main.kiro");
    write_add_fixture_crate(&dir);

    let add = run_kiro(&["add", "kiro_add_fixture"], &dir);
    assert!(
        add.status.success(),
        "kiro add without version should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&add.stdout),
        String::from_utf8_lossy(&add.stderr)
    );

    let manifest = fs::read_to_string(dir.join("kiro.toml")).expect("manifest should be readable");
    assert!(
        manifest.contains(r#"kiro_add_fixture = "0.1.0""#),
        "unversioned add should write the resolved package version, not '*':\n{}",
        manifest
    );
    assert!(
        !manifest.contains(r#"kiro_add_fixture = "*""#),
        "unversioned add must not keep wildcard dependency specs:\n{}",
        manifest
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
import io

import math

io.print(math.add(2, 3))
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
