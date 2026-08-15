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
    pub value: f64,
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

fn write_reexport_fixture_crate(project_dir: &Path) {
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
        r#"mod api;

pub use api::{add, greet, Counter};
pub use api::*;
pub use api::Counter as RenamedCounter;
"#,
    )
    .expect("fixture lib should be written");
    fs::write(
        crate_dir.join("src/api.rs"),
        r#"pub fn add(a: f64, b: f64) -> f64 {
    a + b
}

pub fn greet(name: String) -> String {
    format!("hello {name}")
}

pub fn hidden() -> String {
    "hidden".to_string()
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
}
"#,
    )
    .expect("fixture api should be written");
}

fn write_zova_shaped_fixture_crate(project_dir: &Path) {
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
        r#"mod database;
mod error;

pub use database::{table_info, table_infos, table_name, Database, TableInfo};
pub use error::{Error, Result};
"#,
    )
    .expect("fixture lib should be written");
    fs::write(
        crate_dir.join("src/error.rs"),
        r#"#[derive(Debug)]
pub struct Error;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fixture error")
    }
}

pub type Result<T> = std::result::Result<T, Error>;
"#,
    )
    .expect("fixture error module should be written");
    fs::write(
        crate_dir.join("src/database.rs"),
        r#"use std::path::Path;

use crate::Result;

pub struct CustomPath;

pub struct TableInfo {
    pub name: String,
    pub rows: u64,
}

pub fn table_info() -> TableInfo {
    TableInfo { name: "users".to_string(), rows: 3 }
}

pub fn table_infos() -> Vec<TableInfo> {
    vec![
        table_info(),
        TableInfo { name: "posts".to_string(), rows: 7 },
    ]
}

pub fn table_name(info: TableInfo) -> String {
    info.name
}

pub struct Database {
    label: String,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Database { label: path.as_ref().display().to_string() })
    }

    pub fn create(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Ok(Database { label: path.as_ref().display().to_string() })
    }

    pub fn label(&self) -> Result<String> {
        Ok(self.label.clone())
    }

    pub fn bump(&mut self) -> Result<()> {
        self.label.push('!');
        Ok(())
    }

    pub fn custom(path: impl AsRef<CustomPath>) -> Result<Self> {
        let _ = path;
        Ok(Database { label: "custom".to_string() })
    }

    pub fn generic<T: AsRef<Path>>(path: T) -> Result<Self> {
        Ok(Database { label: path.as_ref().display().to_string() })
    }
}
"#,
    )
    .expect("fixture database module should be written");
}

fn write_private_result_alias_fixture_crate(project_dir: &Path) {
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
        r#"pub struct Error;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fixture error")
    }
}

type Result<T> = std::result::Result<T, Error>;

pub fn add(a: f64, b: f64) -> f64 {
    a + b
}

pub fn hidden_result() -> Result<String> {
    Ok("hidden".to_string())
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
        kiro.contains("rust fn counter_bump(counter: Counter) -> void"),
        "mutable receiver methods should use an ordinary handle parameter:\n{}",
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
fn host_gen_follows_root_named_pub_use_reexports() {
    let dir = temp_project("reexport_bindings");
    link_runtime_and_macros(&dir);
    write_reexport_fixture_crate(&dir);
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
name = "demo_host_gen_reexport"
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
        kiro.contains("rust fn add(a: num, b: num) -> num"),
        "expected re-exported function:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn greet(name: str) -> str"),
        "expected re-exported function:\n{}",
        kiro
    );
    assert!(
        kiro.contains("handle Counter"),
        "expected re-exported handle:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn counter_new(value: num) -> Counter"),
        "expected re-exported constructor:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn counter_value(counter: Counter) -> num"),
        "expected re-exported method:\n{}",
        kiro
    );
    assert!(
        !kiro.contains("hidden"),
        "public module items that are not named re-exports should not be generated:\n{}",
        kiro
    );

    let rust = fs::read_to_string(dir.join("fixture.rs")).expect("fixture.rs should exist");
    assert!(
        rust.contains("kiro_fixture_crate::add"),
        "re-exported free functions should call the public crate-root path:\n{}",
        rust
    );
    assert!(
        rust.contains("kiro_fixture_crate::Counter::new"),
        "re-exported constructors should call the public crate-root path:\n{}",
        rust
    );
    assert!(
        !rust.contains("kiro_fixture_crate::api::"),
        "generated glue must not call private module paths:\n{}",
        rust
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("glob re-exports are unsupported")
            && stdout.contains("alias re-exports are unsupported"),
        "unsupported re-exports should be reported clearly:\n{}",
        stdout
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

#[test]
fn host_gen_collapses_zova_shaped_path_result_self_apis() {
    let dir = temp_project("zova_shaped");
    link_runtime_and_macros(&dir);
    write_zova_shaped_fixture_crate(&dir);
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
name = "demo_host_gen_zova"
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
        kiro.contains("handle Database"),
        "expected database handle:\n{}",
        kiro
    );
    assert!(
        kiro.contains("struct TableInfo")
            && kiro.contains("name: str")
            && kiro.contains("rows: num"),
        "expected public field-only struct value declaration:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn table_info() -> TableInfo"),
        "expected struct-valued function:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn table_infos() -> list TableInfo"),
        "expected list-of-struct function:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn table_name(info: TableInfo) -> str"),
        "expected struct-valued parameter:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn database_open(path: str) -> Database!"),
        "expected AsRef<Path> Result<Self> constructor:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn database_create(path: str) -> Database!"),
        "expected fully-qualified AsRef<std::path::Path> constructor:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn database_label(database: Database) -> str!"),
        "expected fallible immutable method:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn database_bump(database: Database) -> void!"),
        "mutable receiver method should use the ordinary handle parameter:\n{}",
        kiro
    );
    assert!(
        !kiro.contains("database_custom"),
        "custom AsRef target should remain skipped:\n{}",
        kiro
    );
    assert!(
        !kiro.contains("database_generic"),
        "generic AsRef<Path> method should remain skipped:\n{}",
        kiro
    );

    let rust = fs::read_to_string(dir.join("fixture.rs")).expect("fixture.rs should exist");
    assert!(
        rust.contains("kiro_fixture_crate::Database::open(path)"),
        "constructor should call public crate-root path:\n{}",
        rust
    );
    assert!(
        rust.contains("KiroError::message(\"Error\", err.to_string())"),
        "crate-local Result alias should use alias error name:\n{}",
        rust
    );

    let check = run_kiro(&["check", "fixture.kiro"], &dir);
    assert!(
        check.status.success(),
        "generated Zova-shaped Kiro declarations should check\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );

    let glue_check_dir = dir.join("glue_check");
    fs::create_dir_all(glue_check_dir.join("src")).expect("glue check src should be created");
    fs::write(
        glue_check_dir.join("Cargo.toml"),
        r#"[package]
name = "glue_check"
version = "0.1.0"
edition = "2021"

[dependencies]
kiro_fixture_crate = { path = "../fixture_crate" }
kiro_macros = { path = "../kiro_macros" }
kiro_runtime = { path = "../kiro_runtime" }
"#,
    )
    .expect("glue check manifest should be written");
    fs::write(
        glue_check_dir.join("src/lib.rs"),
        r#"use kiro_runtime::{HostResult, KiroError, RuntimeVal};

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../fixture.rs"));
"#,
    )
    .expect("glue check lib should be written");

    fs::write(
        glue_check_dir.join("src/main.rs"),
        r#"use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use kiro_runtime::{HostResult, KiroError, RuntimeVal};

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../fixture.rs"));

struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn main() {
    let database = block_on(database_open(vec![RuntimeVal::from("base")]))
        .expect("database should open");
    block_on(database_bump(vec![database.clone()])).expect("database should mutate");
    let label = block_on(database_label(vec![database])).expect("label should read");
    assert_eq!(label, RuntimeVal::from("base!"));

    let table = block_on(table_info(vec![])).expect("table info should be returned");
    let RuntimeVal::Struct { type_name, fields } = table else {
        panic!("table info should be a struct value");
    };
    assert_eq!(type_name, "TableInfo");
    assert_eq!(fields.get("name"), Some(&RuntimeVal::from("users")));
    assert_eq!(fields.get("rows"), Some(&RuntimeVal::from(3.0)));

    let tables = block_on(table_infos(vec![])).expect("table infos should be returned");
    let RuntimeVal::List(tables) = tables else {
        panic!("table infos should be a list");
    };
    assert_eq!(tables.len(), 2);
    assert!(matches!(tables.first(), Some(RuntimeVal::Struct { type_name, .. }) if type_name == "TableInfo"));

    let input = RuntimeVal::structure(
        "TableInfo",
        [
            ("name".to_string(), RuntimeVal::from("comments")),
            ("rows".to_string(), RuntimeVal::from(11.0)),
        ]
        .into_iter()
        .collect(),
    );
    let name = block_on(table_name(vec![input])).expect("table info should decode");
    assert_eq!(name, RuntimeVal::from("comments"));
}
"#,
    )
    .expect("glue check main should be written");

    let glue_check = Command::new("cargo")
        .args(["check", "--manifest-path"])
        .arg(glue_check_dir.join("Cargo.toml"))
        .output()
        .expect("cargo check should run for generated glue");
    assert!(
        glue_check.status.success(),
        "generated Zova-shaped Rust glue should compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&glue_check.stdout),
        String::from_utf8_lossy(&glue_check.stderr)
    );

    let glue_run = Command::new("cargo")
        .args(["run", "--quiet", "--manifest-path"])
        .arg(glue_check_dir.join("Cargo.toml"))
        .output()
        .expect("generated mutable handle glue should run");
    assert!(
        glue_run.status.success(),
        "generated mutable handle glue should preserve mutation\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&glue_run.stdout),
        String::from_utf8_lossy(&glue_run.stderr)
    );
}

#[test]
fn host_gen_does_not_use_private_result_aliases() {
    let dir = temp_project("private_result_alias");
    link_runtime_and_macros(&dir);
    write_private_result_alias_fixture_crate(&dir);
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
name = "demo_host_gen_private_alias"
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
        "host gen should still succeed for supported items\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let kiro = fs::read_to_string(dir.join("fixture.kiro")).expect("fixture.kiro should exist");
    assert!(
        kiro.contains("rust fn add(a: num, b: num) -> num"),
        "supported function should still be generated:\n{}",
        kiro
    );
    assert!(
        !kiro.contains("hidden_result"),
        "private one-argument Result alias should not be exposed:\n{}",
        kiro
    );
}
