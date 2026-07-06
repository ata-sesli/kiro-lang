use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_project(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "kiro_host_generator_{}_{}_{}",
        name,
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&dir).expect("temp project should be created");
    dir
}

fn link_runtime_and_macros(project_dir: &Path) {
    for crate_name in ["kiro_runtime", "kiro_macros"] {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(crate_name);
        let dst = project_dir.join(crate_name);
        #[cfg(unix)]
        symlink(&src, &dst)
            .unwrap_or_else(|e| panic!("{} symlink should be created: {}", crate_name, e));
        #[cfg(not(unix))]
        {
            fs::create_dir_all(dst.join("src")).expect("crate src dir should be created");
            fs::copy(src.join("Cargo.toml"), dst.join("Cargo.toml"))
                .expect("Cargo.toml should be copied");
            fs::copy(src.join("src/lib.rs"), dst.join("src/lib.rs"))
                .expect("lib.rs should be copied");
        }
    }
}

fn write_fixture_crate(project_dir: &Path) {
    let crate_dir = project_dir.join("fixture_crate");
    fs::create_dir_all(crate_dir.join("src")).expect("fixture src should be created");
    fs::write(
        crate_dir.join("Cargo.toml"),
        r#"[package]
name = "kiro_fixture_crate"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("fixture manifest should be written");
    fs::write(
        crate_dir.join("src/lib.rs"),
        r#"use std::collections::HashMap;

pub fn add(a: f64, b: f64) -> f64 {
    a + b
}

pub fn greet(name: String) -> String {
    format!("hello {name}")
}

pub fn fail(flag: bool) -> Result<String, FixtureError> {
    if flag { Ok("ok".to_string()) } else { Err(FixtureError) }
}

pub struct FixtureError;

impl std::fmt::Display for FixtureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fixture error")
    }
}

pub struct Counter {
    value: f64,
}

impl Counter {
    pub fn new(value: f64) -> Counter {
        Counter { value }
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn bump(&mut self) {
        self.value += 1.0;
    }
}

pub fn labels() -> Vec<String> {
    vec!["a".to_string()]
}

pub fn scores() -> HashMap<String, f64> {
    HashMap::new()
}

pub fn generic<T>(value: T) -> T {
    value
}
"#,
    )
    .expect("fixture lib should be written");
}

fn run_kiro(args: &[&str], current_dir: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
        .args(args)
        .current_dir(current_dir)
        .output()
        .expect("kiro-lang command should run")
}

#[test]
fn host_gen_requires_crate_dependency() {
    let dir = temp_project("missing_dep");
    fs::write(
        dir.join("kiro.toml"),
        r#"[package]
name = "demo"
entry = "main.kiro"

[dependencies]
"#,
    )
    .expect("manifest should be written");

    let output = run_kiro(&["host", "gen", "kiro_fixture_crate"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "missing dependency should fail");
    assert!(
        stderr.contains("Dependency 'kiro_fixture_crate' is not declared in kiro.toml"),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("kiro add kiro_fixture_crate@version"),
        "missing dependency should include add help:\n{}",
        stderr
    );
}

#[test]
fn host_gen_generates_bindings_and_preserves_manual_code() {
    let dir = temp_project("generate_bindings");
    link_runtime_and_macros(&dir);
    write_fixture_crate(&dir);
    fs::write(
        dir.join("kiro.toml"),
        r#"[package]
name = "demo"
entry = "main.kiro"

[dependencies]
kiro_fixture_crate = "0.1.0"
"#,
    )
    .expect("manifest should be written");
    fs::write(
        dir.join("Cargo.toml"),
        r#"[package]
name = "demo_host_gen"
version = "0.1.0"
edition = "2021"

[dependencies]
kiro_fixture_crate = { path = "fixture_crate" }
"#,
    )
    .expect("metadata manifest should be written");
    fs::create_dir_all(dir.join("src")).expect("project src should be created");
    fs::write(dir.join("src/lib.rs"), "").expect("project lib should be written");

    let output = run_kiro(
        &["host", "gen", "kiro_fixture_crate", "--module", "fixture"],
        &dir,
    );
    assert!(
        output.status.success(),
        "host gen should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let kiro = fs::read_to_string(dir.join("fixture.kiro")).expect("fixture.kiro should exist");
    assert!(
        kiro.contains("handle Counter"),
        "expected handle:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn add(a: num, b: num) -> num"),
        "expected add:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn greet(name: str) -> str"),
        "expected greet:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn fail(flag: bool) -> str!"),
        "expected fallible:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn counter_new(value: num) -> Counter"),
        "expected constructor:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn counter_value(counter: Counter) -> num"),
        "expected method:\n{}",
        kiro
    );
    assert!(
        !kiro.contains("counter_bump"),
        "mutable receiver methods should be skipped:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn labels() -> list str"),
        "expected list:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn scores() -> map str num"),
        "expected map:\n{}",
        kiro
    );
    assert!(
        !kiro.contains("generic"),
        "unsupported generic should be skipped:\n{}",
        kiro
    );

    let rust = fs::read_to_string(dir.join("fixture.rs")).expect("fixture.rs should exist");
    assert!(
        rust.contains("mod __kiro_manual_fixture"),
        "module-specific manual module should exist:\n{}",
        rust
    );
    assert!(
        rust.contains("kiro:generated begin"),
        "generated region should exist:\n{}",
        rust
    );
    assert!(
        rust.contains("kiro_fixture_crate::add"),
        "glue should call crate:\n{}",
        rust
    );

    let manual_insert = rust.replace(
        "mod __kiro_manual_fixture {\n    use super::*;\n    use kiro_macros::{kiro_export, kiro_handle, kiro_struct};\n    use std::collections::HashMap;\n}",
        r#"mod __kiro_manual_fixture {
    use super::*;
    use kiro_macros::{kiro_export, kiro_handle, kiro_struct};
    use std::collections::HashMap;

    pub fn kept() -> String { "kept".to_string() }

    #[kiro_export(pure)]
    pub fn manual_add(a: f64, b: f64) -> f64 {
        a + b
    }
}"#,
    );
    fs::write(dir.join("fixture.rs"), manual_insert).expect("manual edit should be written");
    let rerun = run_kiro(
        &["host", "gen", "kiro_fixture_crate", "--module", "fixture"],
        &dir,
    );
    assert!(
        rerun.status.success(),
        "host gen rerun should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rerun.stdout),
        String::from_utf8_lossy(&rerun.stderr)
    );
    let rust = fs::read_to_string(dir.join("fixture.rs")).expect("fixture.rs should exist");
    assert!(
        rust.contains("pub fn kept() -> String"),
        "manual code should survive regeneration:\n{}",
        rust
    );
    assert!(
        rust.contains("pub fn manual_add(args: Vec<RuntimeVal>) -> HostResult"),
        "pure manual export should generate sync glue:\n{}",
        rust
    );
    let kiro = fs::read_to_string(dir.join("fixture.kiro")).expect("fixture.kiro should exist");
    assert!(
        kiro.contains("pure rust fn manual_add(a: num, b: num) -> num"),
        "pure manual export should generate pure rust fn:\n{}",
        kiro
    );

    let check = run_kiro(&["check", "fixture.kiro"], &dir);
    assert!(
        check.status.success(),
        "generated pure rust declaration should parse and check\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn host_gen_uses_module_specific_manual_namespaces() {
    let dir = temp_project("multiple_modules");
    link_runtime_and_macros(&dir);
    write_fixture_crate(&dir);
    fs::write(
        dir.join("kiro.toml"),
        r#"[package]
name = "demo"
entry = "main.kiro"

[dependencies]
kiro_fixture_crate = "0.1.0"
"#,
    )
    .expect("manifest should be written");
    fs::write(
        dir.join("Cargo.toml"),
        r#"[package]
name = "demo_host_gen_multi"
version = "0.1.0"
edition = "2021"

[dependencies]
kiro_fixture_crate = { path = "fixture_crate" }
"#,
    )
    .expect("metadata manifest should be written");
    fs::create_dir_all(dir.join("src")).expect("project src should be created");
    fs::write(dir.join("src/lib.rs"), "").expect("project lib should be written");

    for module in ["fixture_a", "fixture_b"] {
        let output = run_kiro(
            &["host", "gen", "kiro_fixture_crate", "--module", module],
            &dir,
        );
        assert!(
            output.status.success(),
            "host gen should succeed for {module}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let first = fs::read_to_string(dir.join("fixture_a.rs")).expect("fixture_a.rs should exist");
    let second = fs::read_to_string(dir.join("fixture_b.rs")).expect("fixture_b.rs should exist");

    assert!(first.contains("mod __kiro_manual_fixture_a"));
    assert!(second.contains("mod __kiro_manual_fixture_b"));
    assert!(!first.contains("mod manual"));
    assert!(!second.contains("mod manual"));
    assert!(
        !first
            .lines()
            .any(|line| line.starts_with("use kiro_macros")),
        "macro imports should stay inside the unique manual module:\n{}",
        first
    );
}

#[test]
fn host_gen_output_builds_through_kiro_pipeline() {
    let dir = temp_project("build_generated");
    link_runtime_and_macros(&dir);
    fs::write(
        dir.join("kiro.toml"),
        r#"[package]
name = "demo"
entry = "main.kiro"

[dependencies]
dtoa = "1"
"#,
    )
    .expect("manifest should be written");

    let output = run_kiro(&["host", "gen", "dtoa", "--module", "dtoa_bindings"], &dir);
    assert!(
        output.status.success(),
        "host gen should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let kiro =
        fs::read_to_string(dir.join("dtoa_bindings.kiro")).expect("binding module should exist");
    assert!(
        kiro.contains("handle Buffer") && kiro.contains("rust fn buffer_new() -> Buffer"),
        "dtoa should expose Buffer constructor:\n{}",
        kiro
    );

    fs::write(
        dir.join("main.kiro"),
        r#"import io

import dtoa_bindings

var buffer = dtoa_bindings.buffer_new()
io.print(buffer)
"#,
    )
    .expect("main should be written");

    let run = run_kiro(&["run"], &dir);
    assert!(
        run.status.success(),
        "generated host module should build and run\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}
