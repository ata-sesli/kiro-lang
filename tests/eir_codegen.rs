use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use kiro_lang::analysis::{SourceOverlays, analyze_path_with_info};
use kiro_lang::compiler::eir::compile_program;
use kiro_lang::eir::lower_program;

fn lower_source(name: &str, source: &str) -> kiro_lang::eir::EirProgram {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "kiro_eir_codegen_{name}_{}_{}",
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&directory).expect("temporary directory");
    let path = directory.join(PathBuf::from("main.kiro"));
    fs::write(&path, source).expect("source file");
    let analysis = analyze_path_with_info(path, &SourceOverlays::new()).expect("valid source");
    lower_program(&analysis.hir).expect("source lowers to EIR")
}

#[test]
fn rust_backend_emits_functions_slots_blocks_and_direct_calls_from_eir() {
    let program = lower_source(
        "first_slice",
        r#"
pure fn double(value: num) -> num {
    return value * 2
}

fn calculate(limit: num) -> num {
    var current = 0
    loop on (current < limit) {
        current = current + 1
    }
    return double(current)
}

calculate(3)
"#,
    );

    let rust = compile_program(&program).expect("first EIR slice should generate Rust");

    assert!(rust.contains("fn __kiro_eir_f0"));
    assert!(rust.contains("let mut __slots"));
    assert!(rust.contains("match __block"));
    assert!(rust.contains("__kiro_eir_f0"));
    assert!(!rust.contains("fn calculate"));
}

#[test]
fn rust_backend_emits_aggregate_error_and_ownership_operations() {
    let program = lower_source(
        "phase7_values",
        r#"
error Missing = "missing"

struct Item {
    value: num
}

fn values() -> num! {
    var item = Item { value: 40 }
    item.value = item.value + 2
    var items = list num { item.value }
    var pointer = ref items
    var dereferenced = deref pointer
    return dereferenced at 0
}
"#,
    );

    let rust = compile_program(&program).expect("Phase 7 values should generate Rust");

    assert!(rust.contains("__KiroValue::Struct"));
    assert!(rust.contains("__KiroValue::List"));
    assert!(rust.contains("__KiroValue::Address"));
}

#[test]
fn rust_backend_emits_indirect_calls_iterators_and_concurrency_operations() {
    let program = lower_source(
        "phase7_effects",
        r#"
pure fn double(value: num) -> num {
    return value * 2
}

fn worker(channel: pipe num) {
    give channel 20
}

fn effects() -> num {
    var operation = double
    var channel = pipe num
    run worker(channel)
    rest
    var total = operation(take channel)
    loop value in 1..3 {
        total = total + value
    }
    close channel
    return total
}
"#,
    );

    let rust = compile_program(&program).expect("Phase 7 effects should generate Rust");

    assert!(rust.contains("__KiroValue::Function"));
    assert!(rust.contains("tokio::spawn"));
    assert!(rust.contains("tokio::task::yield_now"));
}

#[test]
fn rust_backend_emits_rendezvous_and_language_aggregate_display() {
    let program = lower_source(
        "runtime_semantics",
        r#"
import io

var channel = pipe num 0

io.print(list num { 1, 2 })
"#,
    );

    let rust = compile_program(&program).expect("runtime semantics should generate Rust");

    assert!(rust.contains("__rendezvous_ack"));
    assert!(!rust.contains("async_channel::bounded(0)"));
    assert!(rust.contains("<List len={}>"));
    assert!(rust.contains("<Map len={}>"));
}
