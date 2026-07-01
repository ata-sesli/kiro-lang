use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use kiro_lang::compiler::Compiler;
use kiro_lang::grammar;
use kiro_lang::interpreter::Interpreter;

static KIRO_BUILD_LOCK: Mutex<()> = Mutex::new(());

fn compile_source(source: &str) -> String {
    let program = grammar::parse(source).expect("source should parse");
    let mut compiler = Compiler::new();
    compiler.compile(program, true)
}

fn temp_project(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "kiro_check_diagnostics_{}_{}_{}",
        name,
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&dir).expect("temp project should be created");
    dir
}

fn run_kiro(args: &[&str], current_dir: &PathBuf) -> std::process::Output {
    let _guard = KIRO_BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
        .args(args)
        .current_dir(current_dir)
        .output()
        .expect("kiro-lang command should run")
}

#[test]
fn check_condition_parses_and_compiles() {
    let rust = compile_source(
        r#"
fn main() {
    check 2 > 1
}

main()
"#,
    );

    assert!(
        rust.contains("kiro_check_failed"),
        "generated Rust should contain Kiro check failure handling:\n{}",
        rust
    );
}

#[test]
fn check_optional_message_parses_and_compiles() {
    let rust = compile_source(
        r#"
fn main() {
    check 2 > 1, "math still works"
}

main()
"#,
    );

    assert!(
        rust.contains("math still works"),
        "generated Rust should keep custom check message:\n{}",
        rust
    );
}

#[test]
fn check_is_allowed_inside_pure_function() {
    let rust = compile_source(
        r#"
pure fn positive(x: num) -> num {
    check x > 0, "x must be positive"
    return x
}
"#,
    );

    assert!(
        rust.contains("pub  fn positive"),
        "pure function with check should compile sync:\n{}",
        rust
    );
}

#[test]
fn interpreter_passing_check_is_noop() {
    let program = grammar::parse(
        r#"
check true, "should pass"
"#,
    )
    .expect("source should parse");
    let mut interpreter = Interpreter::new();

    interpreter
        .run(program)
        .expect("passing check should not fail interpreter");
}

#[test]
fn interpreter_failing_check_reports_message() {
    let program = grammar::parse(
        r#"
check false, "x must be positive"
"#,
    )
    .expect("source should parse");
    let mut interpreter = Interpreter::new();

    let err = interpreter
        .run(program)
        .expect_err("failing check should fail interpreter");

    assert!(
        err.contains("Check failed: x must be positive"),
        "unexpected check error: {}",
        err
    );
}

#[test]
fn compiled_failing_check_exits_with_status_one() {
    let dir = temp_project("failing_check");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
fn main() {
    check false, "x must be positive"
}

main()
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["run", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "failing check should exit with status 1\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
    assert!(
        stderr.contains("[KIRO3001:runtime] Check failed: x must be positive"),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn non_bool_check_condition_fails_before_rust_build() {
    let dir = temp_project("non_bool_check");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
check 1
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "non-bool check should fail");
    assert!(
        stderr.contains("Check condition must be bool."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn wrong_argument_count_fails_before_rust_build() {
    let dir = temp_project("wrong_arg_count");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
pure fn add(a: num, b: num) -> num {
    return a + b
}

print add(1)
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "wrong argument count should fail");
    assert!(
        stderr.contains("Wrong argument count for 'add': expected 2, got 1."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("wrong argument count"),
        "diagnostic should label the bad call:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn unknown_function_fails_before_rust_build() {
    let dir = temp_project("unknown_function");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
missing()
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "unknown function should fail");
    assert!(
        stderr.contains("Unknown function 'missing'."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn unknown_variable_fails_before_rust_build() {
    let dir = temp_project("unknown_variable");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
print missing
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "unknown variable should fail");
    assert!(
        stderr.contains("Unknown variable 'missing'."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn unknown_variable_diagnostic_shows_source_location_and_label() {
    let dir = temp_project("unknown_variable_location");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
var count = 1
print coutn
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "unknown variable should fail");
    assert!(
        stderr.contains("--> main.kiro:3:7"),
        "diagnostic should include file, line, and column:\n{}",
        stderr
    );
    assert!(
        stderr.contains("3 | print coutn"),
        "diagnostic should include the source line:\n{}",
        stderr
    );
    assert!(
        stderr.contains("|       ^^^^^ unknown variable"),
        "diagnostic should label the unknown variable:\n{}",
        stderr
    );
    assert!(
        stderr.contains("help: did you mean 'count'?"),
        "diagnostic should suggest the nearest visible binding:\n{}",
        stderr
    );
}

#[test]
fn unknown_function_diagnostic_suggests_known_function() {
    let dir = temp_project("unknown_function_suggestion");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
fn print_total() {
    print "ok"
}

pritn_total()
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "unknown function should fail");
    assert!(
        stderr.contains("help: did you mean 'print_total'?"),
        "diagnostic should suggest the nearest known function:\n{}",
        stderr
    );
}

#[test]
fn bad_pipe_use_fails_before_rust_build() {
    let dir = temp_project("bad_pipe");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
give 1 2
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "bad pipe use should fail");
    assert!(
        stderr.contains("'give' expects a pipe."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("bad give"),
        "diagnostic should label the bad give statement:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn pipe_value_type_mismatch_fails_before_rust_build() {
    let dir = temp_project("pipe_value_type_mismatch");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
fn main() {
    var p = pipe num
    give p "oops"
}

main()
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "bad pipe value should fail");
    assert!(
        stderr.contains("'give' value must be num, got str."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn pure_violation_fails_before_rust_build() {
    let dir = temp_project("pure_violation");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
pure fn log() {
    print "nope"
}
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "pure violation should fail");
    assert!(
        stderr.contains("Pure Function Error: 'print' is forbidden."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("forbidden in pure fn"),
        "diagnostic should label the pure violation:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn immutable_mutation_fails_before_rust_build() {
    let dir = temp_project("immutable_mutation");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
x = 1
x = 2
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "immutable mutation should fail");
    assert!(
        stderr.contains("Cannot mutate immutable variable 'x'."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("immutable variable"),
        "diagnostic should label the immutable binding:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn imported_unknown_function_fails_before_rust_build() {
    let dir = temp_project("imported_unknown_function");
    let main_path = dir.join("main.kiro");
    fs::write(
        dir.join("math.kiro"),
        r#"
pure fn add(a: num, b: num) -> num {
    return a + b
}
"#,
    )
    .expect("math module should be written");
    fs::write(
        &main_path,
        r#"
import math

print math.missing(1)
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "missing imported function should fail"
    );
    assert!(
        stderr.contains("Unknown function 'math.missing'."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn imported_unknown_function_diagnostic_suggests_known_export() {
    let dir = temp_project("imported_unknown_function_suggestion");
    let main_path = dir.join("main.kiro");
    fs::write(
        dir.join("math.kiro"),
        r#"
pure fn read(a: num) -> num {
    return a
}
"#,
    )
    .expect("math module should be written");
    fs::write(
        &main_path,
        r#"
import math

print math.raed(1)
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "unknown imported function should fail"
    );
    assert!(
        stderr.contains("--> main.kiro:4:7"),
        "diagnostic should include call-site location:\n{}",
        stderr
    );
    assert!(
        stderr.contains("help: did you mean 'math.read'?"),
        "diagnostic should suggest known imported function:\n{}",
        stderr
    );
}

#[test]
fn wrong_return_type_fails_before_rust_build() {
    let dir = temp_project("wrong_return_type");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
pure fn name() -> num {
    return "kiro"
}
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "wrong return type should fail");
    assert!(
        stderr.contains("Wrong return type: expected num, got str."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn bad_collection_use_fails_before_rust_build() {
    let dir = temp_project("bad_collection");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
print 1 at 0
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "bad collection use should fail");
    assert!(
        stderr.contains("'at' expects a list or map."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("bad access"),
        "diagnostic should label the bad collection access:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn list_index_type_mismatch_fails_before_rust_build() {
    let dir = temp_project("list_index_type_mismatch");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
fn main() {
    var xs = list num { 1 }
    print xs at "zero"
}

main()
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "bad list index should fail");
    assert!(
        stderr.contains("List index must be num, got str."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn map_key_type_mismatch_fails_before_rust_build() {
    let dir = temp_project("map_key_type_mismatch");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
fn main() {
    var scores = map str num { "ada" 1 }
    print scores at 0
}

main()
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "bad map key should fail");
    assert!(
        stderr.contains("Map key must be str, got num."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn push_value_type_mismatch_fails_before_rust_build() {
    let dir = temp_project("push_value_type_mismatch");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
fn main() {
    var xs = list num { 1 }
    xs push "oops"
}

main()
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "bad pushed value should fail");
    assert!(
        stderr.contains("'push' value must be num, got str."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn effectful_recursion_fails_without_rust_panic_output() {
    let dir = temp_project("effectful_recursion_no_panic");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
fn crawl() {
    crawl()
}

crawl()
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "effectful recursion should fail");
    assert!(
        stderr.contains("Recursive calls are only supported in pure fn."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("thread 'main'") && !stderr.contains("panicked at"),
        "diagnostic should not leak Rust panic output:\n{}",
        stderr
    );
}

#[test]
fn runtime_take_on_closed_pipe_reports_kiro_diagnostic() {
    let dir = temp_project("take_closed_pipe");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
var p = pipe num
close p
print take p
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["run", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr:\n{}", stderr);
    assert!(
        stderr.contains("[KIRO3002:runtime] Pipe is closed; cannot take a value."),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn runtime_give_to_closed_pipe_reports_kiro_diagnostic() {
    let dir = temp_project("give_closed_pipe");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
var p = pipe num
close p
give p 1
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["run", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr:\n{}", stderr);
    assert!(
        stderr.contains("[KIRO3003:runtime] Pipe receiver is closed; cannot give a value."),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn runtime_list_index_out_of_bounds_reports_kiro_diagnostic() {
    let dir = temp_project("list_oob");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
var xs = list num { 1, 2, 3 }
print xs at 5
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["run", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr:\n{}", stderr);
    assert!(
        stderr.contains("[KIRO3004:runtime] List index out of bounds: index 5, length 3."),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn runtime_missing_map_key_reports_kiro_diagnostic() {
    let dir = temp_project("missing_map_key");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
var users = map str num { "alice" 1 }
print users at "user_id"
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["run", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr:\n{}", stderr);
    assert!(
        stderr.contains("[KIRO3005:runtime] Map key not found: \"user_id\"."),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn runtime_empty_address_deref_reports_kiro_diagnostic() {
    let dir = temp_project("empty_adr");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
var p = adr num
print deref p
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["run", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr:\n{}", stderr);
    assert!(
        stderr.contains("[KIRO3006:runtime] Cannot deref an empty address."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("help: Assign it with `ref value` before using `deref`."),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn runtime_host_function_failure_reports_kiro_diagnostic() {
    let dir = temp_project("host_failure");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
rust fn fail() -> str

print fail()
"#,
    )
    .expect("main module should be written");
    fs::write(
        dir.join("main.rs"),
        r#"
pub async fn fail(_args: Vec<kiro_runtime::RuntimeVal>) -> kiro_runtime::HostResult {
    Err(kiro_runtime::KiroError::message("IoError", "disk is gone"))
}
"#,
    )
    .expect("host glue should be written");

    let output = run_kiro(
        &["run", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr:\n{}", stderr);
    assert!(
        stderr.contains("[KIRO3007:runtime] Host function 'fail' failed: IoError: disk is gone."),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn missing_host_glue_fails_before_rust_build() {
    let dir = temp_project("missing_host_glue");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
rust fn read_file(path: str) -> str!
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "missing glue should fail");
    assert!(
        stderr.contains("[KIRO2009:compile] Missing Rust glue for host function 'main.read_file'."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("--> main.kiro:2:1"),
        "diagnostic should point at rust fn declaration:\n{}",
        stderr
    );
    assert!(
        stderr.contains(
            "help: add 'main.rs' with `pub async fn read_file(args: Vec<RuntimeVal>) -> HostResult`"
        ),
        "diagnostic should explain the required glue:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("panic!") && !stderr.contains("error[E"),
        "missing glue should not leak generated Rust internals:\n{}",
        stderr
    );
}

#[test]
fn failable_host_error_still_matches_by_name() {
    let dir = temp_project("failable_host_error_name");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
error NotFound = "missing"

rust fn read_file(path: str) -> str!

fn main() {
    var result = read_file("missing.txt")
    on (result) {
        print result
    } error NotFound {
        print "caught"
    }
}

main()
"#,
    )
    .expect("main module should be written");
    fs::write(
        dir.join("main.rs"),
        r#"
pub async fn read_file(_args: Vec<kiro_runtime::RuntimeVal>) -> kiro_runtime::HostResult {
    Err(kiro_runtime::KiroError::message("NotFound", "missing.txt"))
}
"#,
    )
    .expect("host glue should be written");

    let output = run_kiro(
        &["run", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "failable host error should be caught\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(stdout.contains("caught"), "unexpected stdout:\n{}", stdout);
}

#[test]
fn missing_return_fails_before_rust_build() {
    let dir = temp_project("missing_return");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
fn score() -> num {
    print "not enough"
}
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "missing return should fail");
    assert!(
        stderr.contains("Function 'score' must return num on every path."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn returning_value_from_void_function_fails_before_rust_build() {
    let dir = temp_project("return_value_void");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
fn log() {
    return "nope"
}
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "void return value should fail");
    assert!(
        stderr.contains("Function 'log' returns void but returned a value."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("help: Add `-> str` or remove the returned value."),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn wrong_argument_type_fails_before_rust_build() {
    let dir = temp_project("wrong_arg_type");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
pure fn add(a: num, b: num) -> num {
    return a + b
}

print add("one", 2)
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "wrong argument type should fail");
    assert!(
        stderr.contains("Argument 1 for 'add' must be num, got str."),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn assignment_type_mismatch_fails_before_rust_build() {
    let dir = temp_project("assignment_type");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
var age = 10
age = "old"
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "assignment type mismatch should fail"
    );
    assert!(
        stderr.contains("Cannot assign str to num variable 'age'."),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn unknown_struct_field_fails_before_rust_build() {
    let dir = temp_project("unknown_field");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
struct User { name: str age: num }
var user = User { name: "Ada", age: 36 }
print user.email
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "unknown field should fail");
    assert!(
        stderr.contains("Type User has no field 'email'."),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn forward_declared_struct_field_fails_before_rust_build() {
    let dir = temp_project("forward_unknown_field");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
fn show() {
    var user = User { name: "Ada", age: 36 }
    print user.email
}

struct User { name: str age: num }
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "unknown field should fail");
    assert!(
        stderr.contains("Type User has no field 'email'."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn final_expression_wrong_return_type_fails_before_rust_build() {
    let dir = temp_project("final_expr_return_type");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
fn score() -> num {
    "oops"
}
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "wrong final expression type should fail"
    );
    assert!(
        stderr.contains("Wrong return type: expected num, got str."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn adr_void_deref_fails_before_rust_build() {
    let dir = temp_project("adr_void_deref");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
var p = adr void
print deref p
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "adr void deref should fail");
    assert!(
        stderr.contains("Cannot deref adr void."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "diagnostic should not leak Rust compiler errors:\n{}",
        stderr
    );
}

#[test]
fn invalid_run_target_fails_before_rust_build() {
    let dir = temp_project("invalid_run");
    let main_path = dir.join("main.kiro");
    fs::write(
        &main_path,
        r#"
fn worker() {}

run worker
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(
        &["build", main_path.to_str().unwrap()],
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "invalid run target should fail");
    assert!(
        stderr.contains("'run' expects a function call."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("bad run"),
        "diagnostic should label the invalid run expression:\n{}",
        stderr
    );
    assert!(
        stderr.contains("help: Use `run worker()` instead of `run worker`."),
        "unexpected stderr:\n{}",
        stderr
    );
}
