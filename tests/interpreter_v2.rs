use std::path::PathBuf;
use std::sync::Arc;

use kiro_lang::grammar;
use kiro_lang::interpreter::registry::FunctionEntryKind;
use kiro_lang::interpreter::values::RuntimeVal;
use kiro_lang::interpreter::{HostCallCtx, HostMode, LoadedModule, ModuleLoader, SessionRuntime};
use kiro_lang::ir::IrModule;

fn lower_main(source: &str) -> IrModule {
    let program = grammar::parse(source).expect("source should parse");
    IrModule::lower("main", program)
}

#[test]
fn ir_lowering_collects_function_and_host_signatures() {
    let module = lower_main(
        r#"
rust fn native_add(a: num, b: num) -> num

pure fn add(a: num, b: num) -> num {
    return a + b
}
"#,
    );

    let add = module.function("add").expect("function should be lowered");
    assert!(add.signature.is_pure);
    assert_eq!(add.signature.params.len(), 2);
    assert_eq!(format!("{:?}", add.signature.return_type), "Some(Num)");

    let native = module
        .rust_function("native_add")
        .expect("rust fn should be lowered");
    assert_eq!(native.signature.params.len(), 2);
    assert_eq!(format!("{:?}", native.signature.return_type), "Some(Num)");
}

#[test]
fn v2_session_registers_interpreted_and_host_functions() {
    let module = lower_main(
        r#"
rust fn native_add(a: num, b: num) -> num

fn add(a: num, b: num) -> num {
    return a + b
}
"#,
    );

    let runtime = SessionRuntime::new(module, PathBuf::from("."));

    assert_eq!(
        runtime.registry().entry_kind("main", "add"),
        Some(FunctionEntryKind::InterpretedKiro)
    );
    assert_eq!(
        runtime.registry().entry_kind("main", "native_add"),
        Some(FunctionEntryKind::HostNative)
    );
}

#[test]
fn v2_session_executes_interpreted_function_through_registry() {
    let module = lower_main(
        r#"
fn add(a: num, b: num) -> num {
    return a + b
}
"#,
    );

    let mut runtime = SessionRuntime::new(module, PathBuf::from("."));
    let out = runtime
        .call_function(
            "main",
            "add",
            vec![RuntimeVal::Float(2.0), RuntimeVal::Float(3.0)],
        )
        .expect("function should run");

    assert_eq!(out, RuntimeVal::Float(5.0));
}

#[test]
fn v2_session_executes_registered_host_function_through_registry() {
    let module = lower_main(
        r#"
rust fn native_add(a: num, b: num) -> num

fn main() -> num {
    return native_add(2, 3)
}
"#,
    );

    let mut runtime = SessionRuntime::new(module, PathBuf::from("."));
    runtime.set_host_mode(HostMode::Execute);
    runtime.register_host_fn(
        "main",
        "native_add",
        Arc::new(|_ctx: HostCallCtx, args| {
            let a = match args[0].clone() {
                kiro_runtime::RuntimeVal::Num(n) => n,
                _ => return Err(kiro_runtime::KiroError::new("TypeError")),
            };
            let b = match args[1].clone() {
                kiro_runtime::RuntimeVal::Num(n) => n,
                _ => return Err(kiro_runtime::KiroError::new("TypeError")),
            };
            Ok(kiro_runtime::RuntimeVal::Num(a + b))
        }),
    );

    let out = runtime
        .call_function("main", "main", vec![])
        .expect("main should run");

    assert_eq!(out, RuntimeVal::Float(5.0));
}

#[test]
fn v2_session_runs_top_level_statements() {
    let module = lower_main(
        r#"
fn add(a: num, b: num) -> num {
    return a + b
}

result = add(4, 5)
"#,
    );

    let mut runtime = SessionRuntime::new(module, PathBuf::from("."));
    runtime.run().expect("top-level statements should run");

    assert_eq!(
        runtime.global("result").map(|value| value.data.clone()),
        Some(RuntimeVal::Float(9.0))
    );
}

#[test]
fn v2_session_handles_check_errors() {
    let module = lower_main(
        r#"
fn fail() {
    check false, "v2 check"
}
"#,
    );

    let mut runtime = SessionRuntime::new(module, PathBuf::from("."));
    let err = runtime
        .call_function("main", "fail", vec![])
        .expect_err("failed check should abort execution");

    assert!(err.contains("Check failed: v2 check"), "{err}");
}

#[test]
fn v2_session_handles_errors_pipes_and_collections() {
    let module = lower_main(
        r#"
error NotFound = "missing"

fn guarded() -> num {
    on (NotFound) {
        return 0
    } error NotFound {
        return 7
    }
}

fn pipe_roundtrip() -> num {
    var done = pipe num
    give done 9
    return take done
}

fn collections() -> num {
    var xs = list num { 2, 4, 6 }
    var ages = map str num { "ada" 42 }
    var left = xs at 1
    var right = ages at "ada"
    return left + right
}
"#,
    );

    let mut runtime = SessionRuntime::new(module, PathBuf::from("."));

    assert_eq!(
        runtime
            .call_function("main", "guarded", vec![])
            .expect("error handler should run"),
        RuntimeVal::Float(7.0)
    );
    assert_eq!(
        runtime
            .call_function("main", "pipe_roundtrip", vec![])
            .expect("pipe roundtrip should run"),
        RuntimeVal::Float(9.0)
    );
    assert_eq!(
        runtime
            .call_function("main", "collections", vec![])
            .expect("collections should run"),
        RuntimeVal::Float(46.0)
    );
}

#[derive(Clone)]
struct MemoryLoader;

impl ModuleLoader for MemoryLoader {
    fn load(
        &self,
        module_name: &str,
        _current_dir: &std::path::Path,
    ) -> Result<LoadedModule, String> {
        if module_name != "math" {
            return Err(format!("unexpected module {module_name}"));
        }
        Ok(LoadedModule {
            cache_key: "mem://math".to_string(),
            source: r#"
fn add(a: num, b: num) -> num {
    return a + b
}

answer = 42
"#
            .to_string(),
            base_dir: PathBuf::from("."),
        })
    }
}

#[test]
fn v2_session_imports_modules_and_calls_module_functions() {
    let module = lower_main(
        r#"
import math

fn main() -> num {
    return math.add(math.answer, 8)
}
"#,
    );

    let mut runtime = SessionRuntime::new(module, PathBuf::from("."));
    runtime.set_module_loader(Arc::new(MemoryLoader));

    assert_eq!(
        runtime
            .call_function("main", "main", vec![])
            .expect("imported module function should run"),
        RuntimeVal::Float(50.0)
    );
}
