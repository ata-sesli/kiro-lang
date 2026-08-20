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

pub use database::{consume_bytes, echo_bytes, table_info, table_infos, table_name, Database, TableInfo};
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

pub fn echo_bytes(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

pub fn consume_bytes(data: Vec<u8>) -> usize {
    data.len()
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

fn write_generator_correctness_fixture_crate(project_dir: &Path) {
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
        r#"pub struct OpaqueResult {
    pub optional: Option<String>,
}
"#,
    )
    .expect("fixture lib should be written");
    append_generator_correctness_fixture(&crate_dir);
}

fn write_zova_adapter_fixture_crate(project_dir: &Path) {
    let crate_dir = project_dir.join("zova");
    fs::create_dir_all(crate_dir.join("src")).expect("zova fixture src should be created");
    fs::write(
        crate_dir.join("Cargo.toml"),
        r#"[package]
name = "zova"
version = "0.26.1"
edition = "2021"
"#,
    )
    .expect("zova fixture manifest should be written");
    fs::write(
        crate_dir.join("src/lib.rs"),
        r#"#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Busy,
}

impl Status {
    pub fn name(self) -> String {
        match self {
            Self::Busy => "ZOVA_BUSY".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct Error {
    status: Status,
}

impl Error {
    pub fn status(&self) -> Option<Status> {
        Some(self.status)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZOVA_BUSY: fixture is busy")
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorMetric {
    Cosine,
    L2,
    Dot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorElementType {
    F32,
    F16,
    I8,
}

#[derive(Debug, Clone, Copy)]
pub struct VectorCollectionOptions {
    pub dimensions: u32,
    pub metric: VectorMetric,
    pub element_type: VectorElementType,
}

#[derive(Debug, Clone, Copy)]
pub enum VectorValues<'a> {
    F32(&'a [f32]),
    F16(&'a [u16]),
    I8(&'a [i8]),
}

#[derive(Debug, Clone, PartialEq)]
pub enum VectorValuesOwned {
    F32(Vec<f32>),
    F16(Vec<u16>),
    I8(Vec<i8>),
}

pub struct Vector {
    pub id: String,
    pub values: VectorValuesOwned,
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectId(u64);

pub fn object_id(value: u64) -> ObjectId {
    ObjectId(value)
}

pub fn sample_vector() -> Vector {
    Vector {
        id: "sample".to_string(),
        values: VectorValuesOwned::F32(vec![1.5, 2.5]),
    }
}

pub struct SharedDatabase;

impl SharedDatabase {
    pub fn new() -> Self {
        Self
    }

    pub fn metric_name(&self, options: VectorCollectionOptions) -> String {
        match options.metric {
            VectorMetric::Cosine => "cosine",
            VectorMetric::L2 => "l2",
            VectorMetric::Dot => "dot",
        }
        .to_string()
    }

    pub fn put_vector(&self, values: VectorValues<'_>) -> Result<()> {
        let populated = match values {
            VectorValues::F32(values) => !values.is_empty(),
            VectorValues::F16(values) => !values.is_empty(),
            VectorValues::I8(values) => !values.is_empty(),
        };
        if populated {
            Ok(())
        } else {
            Err(Error { status: Status::Busy })
        }
    }

    pub fn read_object_range(
        &self,
        id: ObjectId,
        offset: u64,
        buffer: &mut [u8],
    ) -> Result<usize> {
        let source = [id.0 as u8, offset as u8, 30, 40];
        let copied = source.len().min(buffer.len());
        buffer[..copied].copy_from_slice(&source[..copied]);
        Ok(copied)
    }

    pub fn fail_busy(&self) -> Result<()> {
        Err(Error { status: Status::Busy })
    }
}
"#,
    )
    .expect("zova fixture source should be written");
}

fn append_generator_correctness_fixture(crate_dir: &Path) {
    let mut source = fs::read_to_string(crate_dir.join("src/lib.rs"))
        .expect("fixture source prefix should exist");
    source.push_str(
        r#"
pub fn opaque_result() -> OpaqueResult {
    OpaqueResult { optional: None }
}

pub fn borrowed_echo(value: &str) -> String {
    value.to_string()
}

pub struct BorrowedRecord<'a> {
    pub value: &'a str,
    pub data: &'a [u8],
}

pub fn borrowed_record_value(record: BorrowedRecord<'_>) -> String {
    record.value.to_string()
}

pub fn borrowed_record_static() -> BorrowedRecord<'static> {
    BorrowedRecord {
        value: "static",
        data: b"static",
    }
}

pub struct GenericRecord<T> {
    pub value: T,
}

pub fn generic_record_value(record: GenericRecord<String>) -> String {
    record.value
}

pub struct BorrowedHandle<'a> {
    value: &'a str,
}

impl<'a> BorrowedHandle<'a> {
    pub fn value(&self) -> String {
        self.value.to_string()
    }
}

pub struct LocalOnly {
    value: std::rc::Rc<()>,
}

impl LocalOnly {
    pub fn new() -> Self {
        Self { value: std::rc::Rc::new(()) }
    }

    pub fn strong_count(&self) -> usize {
        std::rc::Rc::strong_count(&self.value)
    }
}

#[derive(Clone)]
pub struct SharedThing {
    value: std::sync::Arc<()>,
}

impl SharedThing {
    pub fn new() -> Self {
        Self { value: std::sync::Arc::new(()) }
    }

    pub fn strong_count(&self) -> usize {
        std::sync::Arc::strong_count(&self.value)
    }
}

#[derive(Clone, Copy)]
pub struct CopyId(u64);

impl CopyId {
    pub fn into_value(self) -> u64 {
        self.0
    }
}

pub fn make_copy_id(value: u64) -> CopyId {
    CopyId(value)
}

pub fn copy_id_value(id: CopyId) -> u64 {
    id.0
}

pub struct ManualCopyId(u64);

impl Copy for ManualCopyId {}

impl Clone for ManualCopyId {
    fn clone(&self) -> Self {
        *self
    }
}

pub fn make_manual_copy_id(value: u64) -> ManualCopyId {
    ManualCopyId(value)
}

pub fn manual_copy_id_value(id: ManualCopyId) -> u64 {
    id.0
}

pub struct OwnedToken(String);

pub fn make_owned_token(value: String) -> OwnedToken {
    OwnedToken(value)
}

pub fn consume_owned_token(token: OwnedToken) -> usize {
    token.0.len()
}

pub struct SharedWriter {
    bytes: Vec<u8>,
}

impl SharedWriter {
    pub fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    pub fn write(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub fn cancel(self) {}
}
"#,
    );
    fs::write(crate_dir.join("src/lib.rs"), source).expect("fixture lib should be written");
}

fn run_kiro(args: &[&str], current_dir: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
        .args(args)
        .current_dir(current_dir)
        .output()
        .expect("kiro-lang command should run")
}

fn generate_correctness_fixture(name: &str) -> (PathBuf, std::process::Output) {
    let dir = temp_project(name);
    link_runtime_and_macros(&dir);
    write_generator_correctness_fixture_crate(&dir);
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
name = "demo_host_gen_correctness"
version = "0.1.0"
edition = "2021"

[dependencies]
kiro_fixture_crate = { path = "fixture_crate" }
kiro_macros = { path = "kiro_macros" }
kiro_runtime = { path = "kiro_runtime" }
"#,
    )
    .expect("metadata manifest should be written");
    fs::create_dir_all(dir.join("src")).expect("project src should be created");
    fs::write(dir.join("src/lib.rs"), "").expect("project lib should be written");

    let output = run_kiro(
        &["host", "gen", "kiro_fixture_crate", "--module", "fixture"],
        &dir,
    );
    (dir, output)
}

#[test]
fn host_gen_declares_every_emitted_signature_type() {
    let (dir, output) = generate_correctness_fixture("closed_type_graph");
    assert!(
        output.status.success(),
        "host gen should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let kiro = fs::read_to_string(dir.join("fixture.kiro")).expect("fixture.kiro should exist");
    assert!(
        kiro.contains("handle OpaqueResult"),
        "expected fallback handle:\n{kiro}"
    );
    assert!(
        kiro.contains("rust fn opaque_result() -> OpaqueResult"),
        "expected handle-valued function:\n{kiro}"
    );

    let check = run_kiro(&["check", "fixture.kiro"], &dir);
    assert!(
        check.status.success(),
        "every emitted signature type should be declared\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn host_gen_preserves_borrowed_string_parameters() {
    let (dir, output) = generate_correctness_fixture("borrowed_string");
    assert!(output.status.success(), "host gen should succeed");

    let rust = fs::read_to_string(dir.join("fixture.rs")).expect("fixture.rs should exist");
    assert!(
        rust.contains(
            "let value = RuntimeVal::expect_arg(&args, 0, \"borrowed_echo\")?.as_str()?;"
        ),
        "borrowed string should not allocate an owned String:\n{rust}"
    );
    assert!(
        rust.contains("kiro_fixture_crate::borrowed_echo(value)"),
        "borrowed string should be passed directly:\n{rust}"
    );

    fs::write(
        dir.join("src/lib.rs"),
        r#"use kiro_runtime::{HostResult, KiroError, RuntimeVal};

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixture.rs"));
"#,
    )
    .expect("glue-check source should be written");
    let glue_check = Command::new("cargo")
        .args(["check", "--quiet", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .output()
        .expect("cargo check should run for generated glue");
    assert!(
        glue_check.status.success(),
        "borrowed string glue should compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&glue_check.stdout),
        String::from_utf8_lossy(&glue_check.stderr)
    );
}

#[test]
fn host_gen_generates_borrowed_record_input_views() {
    let (dir, output) = generate_correctness_fixture("borrowed_record_input_view");
    assert!(output.status.success(), "host gen should succeed");

    let kiro = fs::read_to_string(dir.join("fixture.kiro")).expect("fixture.kiro should exist");
    assert!(
        kiro.contains("struct BorrowedRecord {\n    value: str\n    data: bytes\n}"),
        "borrowed Rust record should remain an ordinary Kiro struct:\n{kiro}"
    );
    assert!(
        kiro.contains("rust fn borrowed_record_value(record: BorrowedRecord) -> str"),
        "borrowed record input binding should be generated:\n{kiro}"
    );

    let rust = fs::read_to_string(dir.join("fixture.rs")).expect("fixture.rs should exist");
    assert!(
        rust.contains("kiro_fixture_crate::BorrowedRecord { value: __kiro_fields.get(\"value\")"),
        "glue should construct the crate's borrowed input record:\n{rust}"
    );
    assert!(
        rust.contains(".as_str()?, data:"),
        "borrowed record field should view the runtime string without allocation:\n{rust}"
    );
    assert!(
        rust.contains(".as_bytes()?"),
        "borrowed record field should view the runtime bytes without allocation:\n{rust}"
    );

    fs::write(
        dir.join("src/lib.rs"),
        r#"use kiro_runtime::{HostResult, KiroError, RuntimeVal};

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixture.rs"));
"#,
    )
    .expect("glue-check source should be written");
    let glue_check = Command::new("cargo")
        .args(["check", "--quiet", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .output()
        .expect("cargo check should run for generated glue");
    assert!(
        glue_check.status.success(),
        "borrowed record input-view glue should compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&glue_check.stdout),
        String::from_utf8_lossy(&glue_check.stderr)
    );
}

#[test]
fn host_gen_rejects_generic_custom_types_and_borrowed_handles() {
    let (dir, output) = generate_correctness_fixture("generic_custom_types");
    assert!(
        output.status.success(),
        "host gen should keep supported bindings"
    );

    let kiro = fs::read_to_string(dir.join("fixture.kiro")).expect("fixture.kiro should exist");
    assert!(
        !kiro.contains("borrowed_record_static"),
        "borrowed record return leaked:\n{kiro}"
    );
    assert!(
        !kiro.contains("generic_record_value"),
        "generic record leaked:\n{kiro}"
    );
    assert!(
        !kiro.contains("borrowed_handle_value"),
        "borrowed handle leaked:\n{kiro}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("borrowed_record_static: borrowed record returns are unsupported"),
        "borrowed record return skip reason missing:\n{stdout}"
    );
    assert!(
        stdout.contains(
            "generic_record_value: generic or lifetime-bearing custom types are unsupported"
        ),
        "generic record skip reason missing:\n{stdout}"
    );
    assert!(
        stdout.contains(
            "BorrowedHandle::value: generic or lifetime-bearing custom types are unsupported"
        ),
        "borrowed handle skip reason missing:\n{stdout}"
    );
}

#[test]
fn host_gen_keeps_only_thread_safe_handle_payloads() {
    let (dir, output) = generate_correctness_fixture("thread_safe_handles");
    assert!(output.status.success(), "host gen should succeed");

    let kiro = fs::read_to_string(dir.join("fixture.kiro")).expect("fixture.kiro should exist");
    assert!(
        !kiro.contains("handle LocalOnly")
            && !kiro.contains("local_only_new")
            && !kiro.contains("local_only_strong_count"),
        "non-thread-safe handle API should be skipped:\n{kiro}"
    );
    assert!(
        kiro.contains("handle SharedThing")
            && kiro.contains("shared_thing_new")
            && kiro.contains("shared_thing_strong_count"),
        "thread-safe handle API should remain:\n{kiro}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("LocalOnly")
            && stdout.contains("does not satisfy the Kiro handle thread-safety contract"),
        "non-thread-safe handle skip reason missing:\n{stdout}"
    );

    fs::write(
        dir.join("src/lib.rs"),
        r#"use kiro_runtime::{HostResult, KiroError, RuntimeVal};

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixture.rs"));
"#,
    )
    .expect("glue-check source should be written");
    let glue_check = Command::new("cargo")
        .args(["check", "--quiet", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .output()
        .expect("cargo check should run for generated glue");
    assert!(
        glue_check.status.success(),
        "thread-safe filtered glue should compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&glue_check.stdout),
        String::from_utf8_lossy(&glue_check.stderr)
    );
}

#[test]
fn host_gen_copies_copy_handles_and_skips_other_owned_handle_inputs() {
    let (dir, output) = generate_correctness_fixture("copy_handle_inputs");
    assert!(output.status.success(), "host gen should succeed");

    let kiro = fs::read_to_string(dir.join("fixture.kiro")).expect("fixture.kiro should exist");
    assert!(
        kiro.contains("rust fn copy_id_value(id: CopyId) -> num"),
        "Copy handle input should be generated:\n{kiro}"
    );
    assert!(
        kiro.contains("rust fn manual_copy_id_value(id: ManualCopyId) -> num"),
        "manually implemented Copy should be proven by Cargo:\n{kiro}"
    );
    assert!(
        kiro.contains("rust fn copy_id_into_value(copy_id: CopyId) -> num\n"),
        "Copy consuming receiver should operate on a reusable copy:\n{kiro}"
    );
    assert!(
        !kiro.contains("consume_owned_token"),
        "non-Copy owned handle input should be skipped:\n{kiro}"
    );

    let rust = fs::read_to_string(dir.join("fixture.rs")).expect("fixture.rs should exist");
    assert!(
        rust.contains("let id = *RuntimeVal::expect_arg(&args, 0, \"copy_id_value\")?"),
        "Copy handle should be copied out of its shared handle:\n{rust}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(
            "consume_owned_token: by-value handle 'OwnedToken' is unsupported unless it is Copy"
        ),
        "non-Copy owned handle skip reason missing:\n{stdout}"
    );

    fs::write(
        dir.join("src/lib.rs"),
        r#"use kiro_runtime::{HostResult, KiroError, RuntimeVal};

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixture.rs"));
"#,
    )
    .expect("glue-check source should be written");
    let glue_check = Command::new("cargo")
        .args(["check", "--quiet", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .output()
        .expect("cargo check should run for generated glue");
    assert!(
        glue_check.status.success(),
        "Copy-filtered handle glue should compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&glue_check.stdout),
        String::from_utf8_lossy(&glue_check.stderr)
    );
}

#[test]
fn host_gen_generates_one_shot_consuming_receivers() {
    let (dir, output) = generate_correctness_fixture("consuming_receivers");
    assert!(output.status.success(), "host gen should succeed");

    let kiro = fs::read_to_string(dir.join("fixture.kiro")).expect("fixture.kiro should exist");
    assert!(
        kiro.contains("rust fn shared_writer_finish(shared_writer: SharedWriter) -> bytes!"),
        "consuming receiver should be generated as failable:\n{kiro}"
    );
    assert!(
        kiro.contains("rust fn shared_writer_cancel(shared_writer: SharedWriter) -> void!"),
        "void consuming receiver should be generated as failable:\n{kiro}"
    );
    assert!(
        kiro.contains(
            "rust fn shared_writer_write(shared_writer: SharedWriter, bytes: bytes) -> void!"
        ),
        "ordinary methods can observe consumption and must be failable:\n{kiro}"
    );

    let rust = fs::read_to_string(dir.join("fixture.rs")).expect("fixture.rs should exist");
    assert!(
        rust.contains("RuntimeVal::handle(\"SharedWriter\", std::sync::Mutex::new(Some(value)))"),
        "consumable handle should use one-shot payload storage:\n{rust}"
    );
    assert!(
        rust.contains(".take().ok_or_else(|| KiroError::message(\"HandleConsumed\""),
        "consuming receiver should atomically take its payload:\n{rust}"
    );
    assert!(
        rust.contains(".as_mut().ok_or_else(|| KiroError::message(\"HandleConsumed\""),
        "mutable receiver should reject an already consumed handle:\n{rust}"
    );

    fs::write(
        dir.join("src/lib.rs"),
        r#"use kiro_runtime::{HostResult, KiroError, RuntimeVal};

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixture.rs"));
"#,
    )
    .expect("glue-check source should be written");
    let glue_check = Command::new("cargo")
        .args(["check", "--quiet", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .output()
        .expect("cargo check should run for generated glue");
    assert!(
        glue_check.status.success(),
        "one-shot consuming glue should compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&glue_check.stdout),
        String::from_utf8_lossy(&glue_check.stderr)
    );

    fs::write(
        dir.join("src/main.rs"),
        r#"use kiro_runtime::{HostResult, KiroError, RuntimeVal};
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixture.rs"));

struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

fn main() {
    let writer = block_on(shared_writer_new(vec![])).expect("writer should be created");
    block_on(shared_writer_write(vec![
        writer.clone(),
        RuntimeVal::bytes(b"kiro".to_vec()),
    ]))
    .expect("write should succeed");
    let bytes = block_on(shared_writer_finish(vec![writer.clone()]))
        .expect("first finish should succeed");
    assert_eq!(bytes.as_bytes().expect("bytes result"), b"kiro");

    let error = block_on(shared_writer_finish(vec![writer.clone()]))
        .expect_err("second finish should fail");
    assert_eq!(error.name, "HandleConsumed");

    let write_error = block_on(shared_writer_write(vec![
        writer,
        RuntimeVal::bytes(b"again".to_vec()),
    ]))
    .expect_err("write after finish should fail");
    assert_eq!(write_error.name, "HandleConsumed");
}
"#,
    )
    .expect("one-shot runtime probe should be written");
    let runtime_probe = Command::new("cargo")
        .args(["run", "--quiet", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .output()
        .expect("one-shot runtime probe should run");
    assert!(
        runtime_probe.status.success(),
        "one-shot runtime behavior should hold\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&runtime_probe.stdout),
        String::from_utf8_lossy(&runtime_probe.stderr)
    );
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
        kiro.contains("rust fn echo_bytes(data: bytes) -> bytes"),
        "expected borrowed byte slice mapping:\n{}",
        kiro
    );
    assert!(
        kiro.contains("rust fn consume_bytes(data: bytes) -> num"),
        "expected owned byte vector mapping:\n{}",
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
    assert!(
        rust.contains(".as_bytes()?") && rust.contains(".as_bytes()?.to_vec()"),
        "generated glue should borrow slices and copy only for owned byte vectors:\n{}",
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

    let echoed = block_on(echo_bytes(vec![RuntimeVal::bytes([0, 127, 255])]))
        .expect("borrowed bytes should round-trip");
    assert_eq!(echoed.as_bytes().expect("bytes result"), &[0, 127, 255]);
    let length = block_on(consume_bytes(vec![RuntimeVal::bytes([1, 2, 3, 4])]))
        .expect("owned bytes should decode");
    assert_eq!(length, RuntimeVal::from(4.0));
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

#[test]
fn host_gen_adapts_zova_value_enums_vectors_output_buffers_and_errors() {
    let dir = temp_project("zova_adapters");
    link_runtime_and_macros(&dir);
    write_zova_adapter_fixture_crate(&dir);
    fs::write(
        dir.join("kiro.toml"),
        r#"[package]
name = "demo"
entry = "main.kiro"

[dependencies]
zova = "0.26.1"
"#,
    )
    .expect("manifest should be written");
    fs::write(
        dir.join("Cargo.toml"),
        r#"[package]
name = "demo_host_gen_zova_adapters"
version = "0.1.0"
edition = "2021"

[dependencies]
zova = { path = "zova" }
kiro_macros = { path = "kiro_macros" }
kiro_runtime = { path = "kiro_runtime" }
"#,
    )
    .expect("metadata manifest should be written");
    fs::create_dir_all(dir.join("src")).expect("project src should be created");
    fs::write(dir.join("src/lib.rs"), "").expect("project lib should be written");

    let output = run_kiro(&["host", "gen", "zova"], &dir);
    assert!(
        output.status.success(),
        "host gen should support the agreed Zova adapters\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let kiro = fs::read_to_string(dir.join("zova.kiro")).expect("zova.kiro should exist");
    assert!(
        kiro.contains("struct VectorValues {\n    element_type: str\n    values: list num\n}"),
        "numeric vector values should have one Kiro-facing shape:\n{kiro}"
    );
    assert!(
        kiro.contains("metric: str") && kiro.contains("element_type: str"),
        "simple Rust enums in records should map to strings:\n{kiro}"
    );
    assert!(
        kiro.contains("rust fn shared_database_put_vector(shared_database: SharedDatabase, values: VectorValues) -> void!"),
        "borrowed vector representation should become VectorValues:\n{kiro}"
    );
    assert!(
        kiro.contains("rust fn shared_database_read_object_range(shared_database: SharedDatabase, id: ObjectId, offset: num, length: num) -> bytes!"),
        "mutable output buffers should become a length input and bytes output:\n{kiro}"
    );

    let rust = fs::read_to_string(dir.join("zova.rs")).expect("zova.rs should exist");
    assert!(
        rust.contains("InvalidVectorMetric") && rust.contains("zova::VectorMetric::Cosine"),
        "simple enum strings should be validated by generated glue:\n{rust}"
    );
    assert!(
        rust.contains("zova::VectorValues::F32") && rust.contains("zova::VectorValues::I8"),
        "numeric vector values should select a concrete Rust representation:\n{rust}"
    );
    assert!(
        rust.contains("buffer.truncate(copied)") && rust.contains("RuntimeVal::bytes(buffer)"),
        "output buffers should return only the bytes written:\n{rust}"
    );
    assert!(
        rust.contains("__kiro_zova_error") && rust.contains("ZovaBusy"),
        "Zova statuses should become ordinary named Kiro errors:\n{rust}"
    );

    fs::write(
        dir.join("src/main.rs"),
        r#"use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use kiro_runtime::{HostResult, KiroError, RuntimeVal};

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/zova.rs"));

struct NoopWake;
impl Wake for NoopWake { fn wake(self: Arc<Self>) {} }

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

fn main() {
    let database = block_on(shared_database_new(vec![])).expect("database handle");
    let id = block_on(object_id(vec![RuntimeVal::from(7.0)])).expect("object id handle");
    let bytes = block_on(shared_database_read_object_range(vec![
        database.clone(), id, RuntimeVal::from(1.0), RuntimeVal::from(3.0),
    ])).expect("range read");
    assert_eq!(bytes.as_bytes().expect("bytes"), &[7, 1, 30]);

    let options = RuntimeVal::structure("VectorCollectionOptions", [
        ("dimensions".to_string(), RuntimeVal::from(3.0)),
        ("metric".to_string(), RuntimeVal::from("cosine")),
        ("element_type".to_string(), RuntimeVal::from("f32")),
    ].into_iter().collect());
    let metric = block_on(shared_database_metric_name(vec![database.clone(), options]))
        .expect("valid enum strings");
    assert_eq!(metric, RuntimeVal::from("cosine"));

    let invalid_options = RuntimeVal::structure("VectorCollectionOptions", [
        ("dimensions".to_string(), RuntimeVal::from(3.0)),
        ("metric".to_string(), RuntimeVal::from("angular")),
        ("element_type".to_string(), RuntimeVal::from("f32")),
    ].into_iter().collect());
    let enum_error = block_on(shared_database_metric_name(vec![database.clone(), invalid_options]))
        .expect_err("unknown enum string");
    assert_eq!(enum_error.name, "InvalidVectorMetric");

    let vector = RuntimeVal::structure("VectorValues", [
        ("element_type".to_string(), RuntimeVal::from("f32")),
        ("values".to_string(), RuntimeVal::List(vec![RuntimeVal::from(1.0), RuntimeVal::from(2.0)])),
    ].into_iter().collect());
    block_on(shared_database_put_vector(vec![database.clone(), vector])).expect("numeric vector");

    let error = block_on(shared_database_fail_busy(vec![database])).expect_err("busy error");
    assert_eq!(error.name, "ZovaBusy");

    let vector = block_on(sample_vector(vec![])).expect("owned vector");
    let RuntimeVal::Struct { fields, .. } = vector else { panic!("vector struct"); };
    assert!(matches!(fields.get("values"), Some(RuntimeVal::Struct { type_name, .. }) if type_name == "VectorValues"));
}
"#,
    )
    .expect("runtime check source should be written");

    let check = run_kiro(&["check", "zova.kiro"], &dir);
    assert!(
        check.status.success(),
        "generated Kiro should check\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    let glue_run = Command::new("cargo")
        .args(["run", "--quiet", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .output()
        .expect("generated Zova adapter glue should run");
    assert!(
        glue_run.status.success(),
        "generated Zova adapter glue should compile and run\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&glue_run.stdout),
        String::from_utf8_lossy(&glue_run.stderr)
    );
}
