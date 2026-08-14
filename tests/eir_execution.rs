use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kiro_lang::analysis::{SourceOverlays, analyze_path_with_info};
use kiro_lang::eir::{EirProgram, lower_program};
use kiro_lang::grammar;
use kiro_lang::hir::FunctionId;
use kiro_lang::interpreter::eir_runtime::{EirRuntime, EirRuntimeErrorKind};
use kiro_lang::interpreter::values::RuntimeVal;
use kiro_lang::interpreter::{CancellationToken, InterpreterLimits, SessionRuntime};
use kiro_lang::ir::IrModule;

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
