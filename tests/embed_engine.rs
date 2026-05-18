use kiro_lang::engine::{Engine, EngineError, ExecOptions, HostFnSpec, HostMode, Limits, Value};
use kiro_lang::grammar::grammar::KiroType;
use std::sync::{Arc, Mutex};

fn execute_options() -> ExecOptions {
    ExecOptions {
        host_mode: HostMode::Execute,
        limits: Limits::default(),
    }
}

#[test]
fn embedded_engine_executes_registered_host_fn() {
    let mut engine = Engine::builder().build();

    engine
        .register_host_fn(
            HostFnSpec {
                module: "main".to_string(),
                name: "add".to_string(),
                params: vec![KiroType::Num, KiroType::Num],
                ret: KiroType::Num,
                can_error: false,
            },
            |_ctx, args| {
                let a = match &args[0] {
                    Value::Num(n) => *n,
                    _ => return Err(kiro_runtime::KiroError::new("TypeError")),
                };
                let b = match &args[1] {
                    Value::Num(n) => *n,
                    _ => return Err(kiro_runtime::KiroError::new("TypeError")),
                };
                Ok(Value::Num(a + b))
            },
        )
        .expect("register_host_fn should succeed");

    let script = engine
        .compile_module(
            "main",
            r#"
rust fn add(a: num, b: num) -> num

fn main() -> num {
    return add(2, 3)
}
"#,
        )
        .expect("compile should succeed");

    let out = engine
        .run_main(&script, execute_options())
        .expect("run should succeed");

    assert_eq!(out, Value::Num(5.0));
}

#[test]
fn embedded_engine_can_deny_host_calls() {
    let engine = Engine::builder().build();

    let script = engine
        .compile_module(
            "main",
            r#"
rust fn add(a: num, b: num) -> num

fn main() -> num {
    return add(2, 3)
}
"#,
        )
        .expect("compile should succeed");

    let err = engine
        .run_main(
            &script,
            ExecOptions {
                host_mode: HostMode::Deny,
                limits: Limits::default(),
            },
        )
        .expect_err("deny mode should fail");

    match err {
        EngineError::Runtime(msg) => {
            assert!(
                msg.contains("Host call denied"),
                "unexpected message: {msg}"
            );
        }
        other => panic!("unexpected error type: {other}"),
    }
}

#[test]
fn embedded_engine_validates_host_signature() {
    let mut engine = Engine::builder().build();

    engine
        .register_host_fn(
            HostFnSpec {
                module: "main".to_string(),
                name: "greet".to_string(),
                params: vec![KiroType::Str],
                ret: KiroType::Num,
                can_error: false,
            },
            |_ctx, _args| Ok(Value::Num(1.0)),
        )
        .expect("register_host_fn should succeed");

    let script = engine
        .compile_module(
            "main",
            r#"
rust fn greet(name: str) -> str

fn main() -> str {
    return greet("kiro")
}
"#,
        )
        .expect("compile should succeed");

    let err = engine
        .run_main(&script, execute_options())
        .expect_err("signature mismatch should fail");

    match err {
        EngineError::HostRegistration(msg) => {
            assert!(
                msg.contains("return type differs"),
                "unexpected message: {msg}"
            );
        }
        other => panic!("unexpected error type: {other}"),
    }
}

#[test]
fn embedded_engine_does_not_execute_top_level_entry_twice() {
    let mut engine = Engine::builder().build();
    let calls = Arc::new(Mutex::new(0usize));
    let calls_for_host = calls.clone();

    engine
        .register_host_fn(
            HostFnSpec {
                module: "main".to_string(),
                name: "tick".to_string(),
                params: vec![],
                ret: KiroType::Num,
                can_error: false,
            },
            move |_ctx, _args| {
                let mut count = calls_for_host.lock().unwrap();
                *count += 1;
                Ok(Value::Num(*count as f64))
            },
        )
        .expect("register_host_fn should succeed");

    let script = engine
        .compile_module(
            "main",
            r#"
rust fn tick() -> num

fn main() -> num {
    return tick()
}

main()
"#,
        )
        .expect("compile should succeed");

    let out = engine
        .run_main(&script, execute_options())
        .expect("run should succeed");

    assert_eq!(out, Value::Num(1.0));
    assert_eq!(*calls.lock().unwrap(), 1);
}
