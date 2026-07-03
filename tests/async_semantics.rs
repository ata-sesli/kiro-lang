use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use kiro_lang::compiler::Compiler;
use kiro_lang::grammar;

static KIRO_BUILD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
        "kiro_async_semantics_{}_{}_{}",
        name,
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&dir).expect("temp project should be created");
    dir
}

#[test]
fn rest_compiles_to_tokio_yield_now() {
    let rust = compile_source(
        r#"
fn worker() {
    rest
}
"#,
    );

    assert!(
        rust.contains("tokio::task::yield_now().await;"),
        "generated Rust should yield cooperatively:\n{}",
        rust
    );
}

#[test]
fn pure_function_cannot_use_rest() {
    let program = grammar::parse(
        r#"
pure fn worker() {
    rest
}
"#,
    )
    .expect("source should parse");
    let mut compiler = Compiler::new();

    let panic = panic::catch_unwind(AssertUnwindSafe(|| compiler.compile(program, true)))
        .expect_err("pure rest should fail compilation");
    let message = if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = panic.downcast_ref::<&str>() {
        s.to_string()
    } else {
        String::new()
    };

    assert!(
        message.contains("Pure Function Error: 'rest' is forbidden."),
        "unexpected panic message: {}",
        message
    );
}

#[test]
fn effectful_recursion_is_rejected() {
    let program = grammar::parse(
        r#"
fn crawl() {
    crawl()
}
"#,
    )
    .expect("source should parse");
    let mut compiler = Compiler::new();

    let panic = panic::catch_unwind(AssertUnwindSafe(|| compiler.compile(program, true)))
        .expect_err("effectful recursion should fail compilation");
    let message = if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = panic.downcast_ref::<&str>() {
        s.to_string()
    } else {
        String::new()
    };

    assert!(
        message.contains("Recursive calls are only supported in pure fn."),
        "unexpected panic message: {}",
        message
    );
}

#[test]
fn pure_direct_recursion_is_allowed() {
    let rust = compile_source(
        r#"
pure fn count(n: num) -> num {
    on (n <= 0) {
        return 0
    }
    return count(n - 1)
}
"#,
    );

    assert!(
        rust.contains("pub  fn count"),
        "pure function should compile sync"
    );
}

#[test]
fn effectful_mutual_recursion_is_rejected() {
    let program = grammar::parse(
        r#"
fn a() {
    b()
}

fn b() {
    a()
}
"#,
    )
    .expect("source should parse");
    let mut compiler = Compiler::new();

    let panic = panic::catch_unwind(AssertUnwindSafe(|| compiler.compile(program, true)))
        .expect_err("effectful mutual recursion should fail compilation");
    let message = if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = panic.downcast_ref::<&str>() {
        s.to_string()
    } else {
        String::new()
    };

    assert!(
        message.contains("Recursive calls are only supported in pure fn."),
        "unexpected panic message: {}",
        message
    );
}

#[test]
fn run_self_recursion_is_rejected() {
    let program = grammar::parse(
        r#"
fn worker() {
    run worker()
}
"#,
    )
    .expect("source should parse");
    let mut compiler = Compiler::new();

    let panic = panic::catch_unwind(AssertUnwindSafe(|| compiler.compile(program, true)))
        .expect_err("run self recursion should fail compilation");
    let message = if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = panic.downcast_ref::<&str>() {
        s.to_string()
    } else {
        String::new()
    };

    assert!(
        message.contains("Recursive calls are only supported in pure fn."),
        "unexpected panic message: {}",
        message
    );
}

#[test]
fn imported_pure_function_compiles_without_await() {
    let _guard = KIRO_BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_project("imported_pure");
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
import io

import math

fn main() {
    io.print(math.add(1, 2))
}

main()
"#,
    )
    .expect("main module should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
        .arg("build")
        .arg(main_path)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("kiro-lang build should run");

    assert!(
        output.status.success(),
        "imported pure module should compile successfully\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let generated =
        fs::read_to_string(".kiro/build/src/main.rs").expect("generated main Rust should exist");
    assert!(
        generated.contains("math::add")
            && !generated.contains("math::add((1.0).clone(), (2.0).clone()).await"),
        "imported pure call should not be awaited:\n{}",
        generated
    );
}

#[test]
fn imported_effectful_function_compiles_with_await() {
    let _guard = KIRO_BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_project("imported_effectful");
    let main_path = dir.join("main.kiro");
    fs::write(
        dir.join("math.kiro"),
        r#"
fn add(a: num, b: num) -> num {
    return a + b
}
"#,
    )
    .expect("math module should be written");
    fs::write(
        &main_path,
        r#"
import io

import math

fn main() {
    io.print(math.add(1, 2))
}

main()
"#,
    )
    .expect("main module should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
        .arg("build")
        .arg(main_path)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("kiro-lang build should run");

    assert!(
        output.status.success(),
        "imported effectful module should compile successfully\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let generated =
        fs::read_to_string(".kiro/build/src/main.rs").expect("generated main Rust should exist");
    assert!(
        generated.contains("math::add((1.0).clone(), (2.0).clone()).await"),
        "imported effectful call should be awaited:\n{}",
        generated
    );
}

#[test]
fn pure_function_cannot_call_imported_effectful_function() {
    let _guard = KIRO_BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_project("pure_imports_effectful");
    let main_path = dir.join("main.kiro");
    fs::write(
        dir.join("math.kiro"),
        r#"
fn add(a: num, b: num) -> num {
    return a + b
}
"#,
    )
    .expect("math module should be written");
    fs::write(
        &main_path,
        r#"
import io

import math

pure fn calculate() -> num {
    return math.add(1, 2)
}

fn main() {
    io.print(calculate())
}

main()
"#,
    )
    .expect("main module should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
        .arg("build")
        .arg(main_path)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("kiro-lang build should run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "pure imported effectful call should fail\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stderr.contains("Pure function cannot call impure/async function")
            && stderr.contains("math.add"),
        "unexpected stderr:\n{}",
        stderr
    );
}
