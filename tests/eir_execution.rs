use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kiro_lang::analysis::{SourceOverlays, analyze_path_with_info};
use kiro_lang::eir::{EirProgram, lower_program};
use kiro_lang::grammar;
use kiro_lang::hir::FunctionId;
use kiro_lang::interpreter::eir_runtime::{EirRuntime, EirRuntimeErrorKind};
use kiro_lang::interpreter::values::RuntimeVal;
use kiro_lang::interpreter::{CancellationToken, HostMode, InterpreterLimits, SessionRuntime};
use kiro_lang::ir::IrModule;
use std::sync::Arc;

fn temp_source(name: &str, source: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "kiro_eir_execution_{name}_{}_{}",
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&dir).expect("temporary project should be created");
    let path = dir.join("main.kiro");
    fs::write(&path, source).expect("test source should be written");
    path
}

fn lower_source(name: &str, source: &str) -> (EirProgram, FunctionId) {
    let path = temp_source(name, source);
    fs::write(path.with_extension("rs"), "// test host glue\n")
        .expect("host glue marker should be written");
    let analysis = analyze_path_with_info(path, &SourceOverlays::new())
        .expect("source should analyze before execution");
    let function = analysis.hir.modules[0].functions[0].id;
    let program = lower_program(&analysis.hir).expect("supported source should lower to EIR");
    (program, function)
}

fn legacy_call(source: &str, function: &str, args: Vec<RuntimeVal>) -> RuntimeVal {
    let syntax = grammar::parse(source).expect("source should parse for legacy oracle");
    let module = IrModule::lower("main", syntax);
    let mut runtime = SessionRuntime::new(module, PathBuf::from("."));
    runtime
        .call_function("main", function, args)
        .expect("legacy oracle should execute")
}

#[test]
fn eir_executes_typed_primitives_and_matches_the_tree_interpreter() {
    let source = r#"
fn calculate(left: num, right: num, label: str) -> str {
    var sum = left + right
    var total = sum * 2
    on (total >= 10) {
        return label + "!"
    } off {
        return "small"
    }
}
"#;
    let (program, function) = lower_source("primitives", source);
    let args = vec![
        RuntimeVal::Float(2.0),
        RuntimeVal::Float(3.0),
        RuntimeVal::String("ready".to_string()),
    ];
    let expected = legacy_call(source, "calculate", args.clone());
    let mut runtime = EirRuntime::new(&program).expect("verified EIR should create a runtime");

    let actual = runtime
        .call_function(function, args)
        .expect("EIR function should execute");

    assert_eq!(actual, expected);
    assert_eq!(actual, RuntimeVal::String("ready!".to_string()));
}

#[test]
fn eir_executes_loops_branches_and_direct_calls() {
    let source = r#"
fn count(limit: num) -> num {
    var current = 0
    loop on (current < limit) {
        current = current + 1
        on (current == 2) {
            continue
        }
        on (current == 4) {
            break
        }
    }
    return current
}

fn main() -> num {
    return count(10)
}
"#;
    let path = temp_source("control_flow", source);
    let analysis = analyze_path_with_info(path, &SourceOverlays::new()).expect("source analysis");
    let main = analysis.hir.modules[0]
        .function("main")
        .expect("main function")
        .id;
    let program = lower_program(&analysis.hir).expect("supported source should lower");
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");

    assert_eq!(
        runtime.call_function(main, Vec::new()).expect("main runs"),
        RuntimeVal::Float(4.0)
    );
}

#[test]
fn eir_uses_an_iterative_frame_stack_for_recursion_and_enforces_depth() {
    let source = r#"
pure fn factorial(value: num) -> num {
    on (value <= 1) {
        return 1
    } off {
        return value * factorial(value - 1)
    }
}
"#;
    let (program, function) = lower_source("recursion", source);
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");
    assert_eq!(
        runtime
            .call_function(function, vec![RuntimeVal::Float(5.0)])
            .expect("recursive function should run"),
        RuntimeVal::Float(120.0)
    );

    let mut limited = EirRuntime::new(&program).expect("verified runtime");
    limited.set_limits(InterpreterLimits {
        max_steps: None,
        max_call_depth: Some(3),
        timeout: None,
    });
    let error = limited
        .call_function(function, vec![RuntimeVal::Float(5.0)])
        .expect_err("recursive call should hit the configured depth");
    assert!(matches!(
        error.kind,
        EirRuntimeErrorKind::CallDepthExceeded { limit: 3, .. }
    ));
    assert!(error.anchor.end() > error.anchor.start());

    limited.set_limits(InterpreterLimits {
        max_steps: None,
        max_call_depth: Some(10),
        timeout: None,
    });
    assert_eq!(
        limited
            .call_function(function, vec![RuntimeVal::Float(3.0)])
            .expect("runtime should be reusable after a failed call"),
        RuntimeVal::Float(6.0)
    );
}

#[test]
fn eir_runs_module_initializers_and_enforces_step_limits_at_anchors() {
    let source = r#"
fn noop() {
    return
}

noop()
"#;
    let (program, _) = lower_source("initializer", source);
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");
    runtime
        .run_initializers()
        .expect("synthetic module initializer should run");
    assert!(runtime.step_count() > 0);

    let mut limited = EirRuntime::new(&program).expect("verified runtime");
    limited.set_limits(InterpreterLimits {
        max_steps: Some(0),
        max_call_depth: None,
        timeout: None,
    });
    let error = limited
        .run_initializers()
        .expect_err("initializer should hit step limit");
    assert!(matches!(
        error.kind,
        EirRuntimeErrorKind::StepLimitExceeded { limit: 0, .. }
    ));
    assert!(error.anchor.end() > error.anchor.start());
}

#[test]
fn eir_rejects_an_invalid_entry_function_without_panicking() {
    let source = r#"
fn answer() -> num {
    return 42
}
"#;
    let (program, _) = lower_source("invalid_entry_function", source);
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.call_function(FunctionId::new(u32::MAX), Vec::new())
    }));

    let result = outcome.expect("invalid entry function IDs must not panic");
    let error = result.expect_err("invalid entry function IDs must be rejected");
    assert!(error.to_string().contains("invalid function"));
}

#[test]
fn eir_cooperatively_cancels_an_active_loop() {
    let source = r#"
fn spin() {
    var current = 0
    loop on (true) {
        current = current + 1
    }
}
"#;
    let (program, function) = lower_source("cancellation", source);
    let cancellation = CancellationToken::new();
    let runtime_cancellation = cancellation.clone();
    let execution = std::thread::spawn(move || {
        let mut runtime = EirRuntime::new(&program).expect("verified runtime");
        runtime.set_cancellation_token(runtime_cancellation);
        runtime.set_limits(InterpreterLimits {
            max_steps: None,
            max_call_depth: None,
            timeout: Some(Duration::from_secs(1)),
        });
        runtime.call_function(function, Vec::new())
    });

    std::thread::sleep(Duration::from_millis(10));
    cancellation.cancel();

    let error = execution
        .join()
        .expect("EIR execution thread should not panic")
        .expect_err("cancelled EIR execution must stop");
    assert!(matches!(error.kind, EirRuntimeErrorKind::Cancelled));
    assert!(error.anchor.end() > error.anchor.start());
}

#[test]
fn eir_executes_aggregate_construction_access_mutation_and_length() {
    let source = r#"
struct User {
    name: str
    age: num
}

fn aggregate() -> num {
    var user = User { name: "Ada", age: 40 }
    user.age = user.age + 2
    var xs = list num { 1, 2, 3 }
    xs push 4
    var ages = map str num { "ada" 40 }
    return len xs + xs at 1 + xs at 3 + ages at "ada" + user.age
}
"#;
    let (program, function) = lower_source("aggregates", source);
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");

    let value = runtime
        .call_function(function, Vec::new())
        .expect("aggregate function should execute");

    assert_eq!(value, RuntimeVal::Float(92.0));
}

#[test]
fn eir_runs_imported_initializers_before_persisting_module_globals() {
    let main_path = temp_source(
        "modules",
        r#"
import math

var cached = math.answer()

fn read() -> num {
    return cached
}
"#,
    );
    fs::write(
        main_path
            .parent()
            .expect("temporary module directory")
            .join("math.kiro"),
        r#"
var base = 40

fn answer() -> num {
    return base + 2
}
"#,
    )
    .expect("imported module should be written");
    let analysis = analyze_path_with_info(&main_path, &SourceOverlays::new())
        .expect("module graph should analyze");
    let read = analysis
        .hir
        .module("main")
        .expect("main HIR module")
        .function("read")
        .expect("read function")
        .id;
    let program = lower_program(&analysis.hir).expect("module globals should lower");
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");

    runtime
        .run_initializers()
        .expect("module initializers should execute in dependency order");
    let value = runtime
        .call_function(read, Vec::new())
        .expect("module global should persist after initialization");

    assert_eq!(value, RuntimeVal::Float(42.0));
}

#[test]
fn eir_dispatches_declared_error_clauses() {
    let source = r#"
error NotFound = "missing"

fn guarded() -> num {
    on (NotFound) {
        return 0
    } error NotFound {
        return 7
    }
    return 9
}
"#;
    let (program, function) = lower_source("declared_errors", source);
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");

    let value = runtime
        .call_function(function, Vec::new())
        .expect("matching declared error clause should handle the value");

    assert_eq!(value, RuntimeVal::Float(7.0));
}

#[test]
fn eir_propagates_failable_function_values_through_direct_calls() {
    let source = r#"
error NotFound = "missing"

fn maybe_fail(code: num) -> str! {
    on (code == 1) {
        return NotFound
    }
    return "ok"
}

fn process(code: num) -> str! {
    var result = maybe_fail(code)
    on (result) {
        return result
    } error NotFound {
        return NotFound
    }
    return "unreachable"
}
"#;
    let path = temp_source("error_propagation", source);
    let analysis = analyze_path_with_info(path, &SourceOverlays::new()).expect("source analysis");
    let process = analysis.hir.modules[0]
        .function("process")
        .expect("process function")
        .id;
    let program = lower_program(&analysis.hir).expect("failable calls should lower");
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");

    assert_eq!(
        runtime
            .call_function(process, vec![RuntimeVal::Float(0.0)])
            .expect("successful failable call"),
        RuntimeVal::String("ok".to_string())
    );
    assert!(matches!(
        runtime
            .call_function(process, vec![RuntimeVal::Float(1.0)])
            .expect("declared errors are values"),
        RuntimeVal::Error(_, _)
    ));
}

#[test]
fn eir_check_failure_keeps_its_source_anchor() {
    let source = r#"
fn checked(flag: bool) -> num! {
    check flag, "boom"
    return 9
}
"#;
    let (program, function) = lower_source("check_failure", source);
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");

    assert_eq!(
        runtime
            .call_function(function, vec![RuntimeVal::Bool(true)])
            .expect("successful check"),
        RuntimeVal::Float(9.0)
    );
    let error = runtime
        .call_function(function, vec![RuntimeVal::Bool(false)])
        .expect_err("failed check should stop execution");
    assert!(matches!(
        error.kind,
        EirRuntimeErrorKind::CheckFailed(ref message) if message == "boom"
    ));
    assert!(error.anchor.end() > error.anchor.start());
}

#[test]
fn eir_executes_move_address_reference_and_dereference() {
    let source = r#"
fn ownership() -> num {
    var value = 5
    var moved = move value
    var pointer = adr num
    pointer = ref moved
    return deref pointer
}
"#;
    let (program, function) = lower_source("ownership", source);
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");

    let value = runtime
        .call_function(function, Vec::new())
        .expect("ownership operations should execute");

    assert_eq!(value, RuntimeVal::Float(5.0));
}

#[test]
fn eir_executes_function_values_and_indirect_calls() {
    let source = r#"
pure fn double(value: num) -> num {
    return value * 2
}

fn apply() -> num {
    var operation = double
    return operation(21)
}
"#;
    let (program, _) = lower_source("function_values", source);
    let function = program
        .functions
        .iter()
        .find(|function| function.name == "apply")
        .expect("apply function")
        .id;
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");

    assert_eq!(
        runtime
            .call_function(function, Vec::new())
            .expect("apply runs"),
        RuntimeVal::Float(42.0)
    );
}

#[test]
fn eir_executes_registered_host_calls_and_honors_host_mode() {
    let source = r#"
rust fn add(left: num, right: num) -> num

fn calculate() -> num {
    return add(20, 22)
}
"#;
    let (program, function) = lower_source("host_call", source);
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");
    runtime.register_host_fn(
        "main",
        "add",
        Arc::new(|_, args| {
            let [
                kiro_runtime::RuntimeVal::Num(left),
                kiro_runtime::RuntimeVal::Num(right),
            ] = args.as_slice()
            else {
                return Err(kiro_runtime::KiroError::new("TypeError"));
            };
            Ok(kiro_runtime::RuntimeVal::Num(left + right))
        }),
    );
    runtime.set_host_mode(HostMode::Execute);
    assert_eq!(
        runtime
            .call_function(function, Vec::new())
            .expect("host call runs"),
        RuntimeVal::Float(42.0)
    );

    runtime.set_host_mode(HostMode::Deny);
    let error = runtime
        .call_function(function, Vec::new())
        .expect_err("deny mode rejects host calls");
    assert!(error.to_string().contains("host call denied"));
}

#[test]
fn eir_executes_pipe_rest_and_spawn_effects() {
    let source = r#"
fn worker(channel: pipe num) {
    give channel 42
}

fn main() -> num {
    var channel = pipe num
    run worker(channel)
    rest
    return take channel
}
"#;
    let (program, _) = lower_source("concurrency", source);
    let function = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main function")
        .id;
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");

    assert_eq!(
        runtime
            .call_function(function, Vec::new())
            .expect("main runs"),
        RuntimeVal::Float(42.0)
    );
}

#[test]
fn eir_executes_range_list_and_filtered_iterator_loops() {
    let source = r#"
fn iterate() -> num {
    var total = 0
    loop value in 1..6 per 2 {
        total = total + value
    }
    loop value in list num { 2, 3, 4 } on (value > 2) {
        total = total + value
    } off {
        total = total + 10
    }
    return total
}
"#;
    let (program, function) = lower_source("iterator_loops", source);
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");

    assert_eq!(
        runtime
            .call_function(function, Vec::new())
            .expect("loops run"),
        RuntimeVal::Float(26.0)
    );
}

#[test]
fn eir_persists_pushes_to_module_global_lists() {
    let source = r#"
var values = list num { 1 }

fn append() {
    values push 2
}

fn count() -> num {
    return len values
}
"#;
    let path = temp_source("global_push", source);
    let analysis = analyze_path_with_info(path, &SourceOverlays::new()).expect("source analysis");
    let module = &analysis.hir.modules[0];
    let append = module.function("append").expect("append function").id;
    let count = module.function("count").expect("count function").id;
    let program = lower_program(&analysis.hir).expect("global push should lower");
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");

    runtime.run_initializers().expect("globals initialize");
    runtime
        .call_function(append, Vec::new())
        .expect("append should execute");

    assert_eq!(
        runtime
            .call_function(count, Vec::new())
            .expect("count should execute"),
        RuntimeVal::Float(2.0)
    );
}

#[test]
fn eir_close_closes_every_clone_without_consuming_the_slot() {
    let source = r#"
fn close_clone() {
    var original = pipe num
    var alias = original
    close original
    give alias 1
}
"#;
    let (program, function) = lower_source("close_clone", source);
    let mut runtime = EirRuntime::new(&program).expect("verified runtime");

    let error = runtime
        .call_function(function, Vec::new())
        .expect_err("giving through an alias of a closed pipe must fail");

    assert!(matches!(error.kind, EirRuntimeErrorKind::PipeClosed));
}
