use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static KIRO_BUILD_LOCK: Mutex<()> = Mutex::new(());

fn temp_project(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after UNIX_EPOCH")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "kiro_responsibility_split_{}_{}_{}",
        name,
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&dir).expect("temp project should be created");
    dir
}

fn link_runtime(project_dir: &Path) {
    let runtime_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("kiro_runtime");
    let runtime_dst = project_dir.join("kiro_runtime");
    #[cfg(unix)]
    symlink(&runtime_src, &runtime_dst).expect("kiro_runtime symlink should be created");
    #[cfg(not(unix))]
    {
        fs::create_dir_all(runtime_dst.join("src"))
            .expect("kiro_runtime src dir should be created");
        fs::copy(
            runtime_src.join("Cargo.toml"),
            runtime_dst.join("Cargo.toml"),
        )
        .expect("kiro_runtime Cargo.toml should be copied");
        fs::copy(
            runtime_src.join("src/lib.rs"),
            runtime_dst.join("src/lib.rs"),
        )
        .expect("kiro_runtime lib.rs should be copied");
    }
}

fn write_manifest(dir: &Path, entry: &str) {
    fs::write(
        dir.join("kiro.toml"),
        format!(
            r#"[package]
name = "demo"
entry = "{}"

[dependencies]
"#,
            entry
        ),
    )
    .expect("manifest should be written");
}

fn run_kiro(args: &[&str], current_dir: &Path) -> std::process::Output {
    let _guard = KIRO_BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
        .args(args)
        .current_dir(current_dir)
        .output()
        .expect("kiro-lang command should run")
}

fn assert_success(output: &std::process::Output, context: &str) {
    assert!(
        output.status.success(),
        "{}\nstdout:\n{}\nstderr:\n{}",
        context,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn run_uses_compiled_path_without_interpreter_prepass() {
    let dir = temp_project("run_compiled");
    link_runtime(&dir);
    fs::write(
        dir.join("main.kiro"),
        "import io\n\nio.print(\"compiled only\")\n",
    )
    .expect("main module should be written");

    let output = run_kiro(&["run", "main.kiro"], &dir);

    assert_success(&output, "compiled run should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("INTERPRETER"),
        "compiled run should not print interpreter banner:\n{}",
        stdout
    );
    assert_eq!(
        stdout.matches("compiled only").count(),
        1,
        "compiled run should execute the script once:\n{}",
        stdout
    );
}

#[test]
fn bare_file_invocation_uses_compiled_path_without_interpreter_prepass() {
    let dir = temp_project("bare_compiled");
    link_runtime(&dir);
    fs::write(
        dir.join("main.kiro"),
        "import io\n\nio.print(\"bare compiled\")\n",
    )
    .expect("main module should be written");

    let output = run_kiro(&["main.kiro"], &dir);

    assert_success(&output, "bare file invocation should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("INTERPRETER"),
        "bare invocation should not print interpreter banner:\n{}",
        stdout
    );
    assert_eq!(
        stdout.matches("bare compiled").count(),
        1,
        "bare invocation should execute the script once:\n{}",
        stdout
    );
}

#[test]
fn no_interpret_flag_is_removed_from_run() {
    let dir = temp_project("removed_flag");
    fs::write(dir.join("main.kiro"), "print \"unused\"\n").expect("main module should be written");

    let output = run_kiro(&["run", "main.kiro", "--no-interpret"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "--no-interpret should be rejected"
    );
    assert!(
        stderr.contains("--no-interpret"),
        "diagnostic should mention removed flag:\n{}",
        stderr
    );
}

#[test]
fn no_interpret_flag_is_removed_from_bare_invocation() {
    let dir = temp_project("removed_bare_flag");
    fs::write(dir.join("main.kiro"), "print \"unused\"\n").expect("main module should be written");

    let output = run_kiro(&["main.kiro", "--no-interpret"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "--no-interpret should be rejected"
    );
    assert!(
        stderr.contains("--no-interpret"),
        "diagnostic should mention removed flag:\n{}",
        stderr
    );
}

#[test]
fn interpret_executes_with_v2_interpreter() {
    let dir = temp_project("interpret");
    fs::write(
        dir.join("main.kiro"),
        "import io\n\nio.print(\"interpreted\")\n",
    )
    .expect("main module should be written");

    let output = run_kiro(&["interpret", "main.kiro"], &dir);

    assert_success(&output, "interpret should execute directly");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("interpreted"),
        "interpreter stdout should include script output:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn interpret_failed_check_reports_source_span() {
    let dir = temp_project("interpret_check_span");
    fs::write(dir.join("main.kiro"), "check false, \"boom\"\n")
        .expect("main module should be written");

    let output = run_kiro(&["interpret", "main.kiro"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1), "stderr:\n{}", stderr);
    assert!(
        stderr.contains("[KIRO3001:runtime] Check failed: boom"),
        "stderr should show runtime check diagnostic:\n{}",
        stderr
    );
    assert!(
        stderr.contains("main.kiro:1:1"),
        "stderr should include exact check location:\n{}",
        stderr
    );
    assert!(
        stderr.contains("check false"),
        "stderr should include source line:\n{}",
        stderr
    );
}

#[test]
fn interpret_runtime_at_and_deref_errors_report_source_spans() {
    let dir = temp_project("interpret_runtime_spans");
    fs::write(
        dir.join("list.kiro"),
        "import io\n\nvar xs = list num { 1 }\nio.print(xs at 4)\n",
    )
    .expect("list module should be written");

    let output = run_kiro(&["interpret", "list.kiro"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr:\n{}", stderr);
    assert!(
        stderr.contains("[KIRO3004:runtime] List index out of bounds: index 4, length 1."),
        "stderr should show list diagnostic:\n{}",
        stderr
    );
    assert!(
        stderr.contains("list.kiro:4:10") && stderr.contains("xs at 4"),
        "stderr should point at the at expression:\n{}",
        stderr
    );

    fs::write(
        dir.join("map.kiro"),
        "import io\n\nvar users = map str num { \"ada\" 1 }\nio.print(users at \"grace\")\n",
    )
    .expect("map module should be written");

    let output = run_kiro(&["interpret", "map.kiro"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr:\n{}", stderr);
    assert!(
        stderr.contains("[KIRO3005:runtime] Map key not found: \"grace\"."),
        "stderr should show map diagnostic:\n{}",
        stderr
    );
    assert!(
        stderr.contains("map.kiro:4:10") && stderr.contains("users at \"grace\""),
        "stderr should point at the map access expression:\n{}",
        stderr
    );

    fs::write(
        dir.join("deref.kiro"),
        "import io\n\nvar p = adr num\nio.print(deref p)\n",
    )
    .expect("deref module should be written");

    let output = run_kiro(&["interpret", "deref.kiro"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr:\n{}", stderr);
    assert!(
        stderr.contains("[KIRO3006:runtime] Cannot deref an empty address."),
        "stderr should show deref diagnostic:\n{}",
        stderr
    );
    assert!(
        stderr.contains("deref.kiro:4:10") && stderr.contains("deref p"),
        "stderr should point at the deref expression:\n{}",
        stderr
    );
    assert!(
        stderr.contains("help: Assign it with `ref value` before using `deref`."),
        "stderr should include deref help:\n{}",
        stderr
    );
}

#[test]
fn interpret_without_file_uses_manifest_entry() {
    let dir = temp_project("interpret_manifest");
    write_manifest(&dir, "main.kiro");
    fs::write(
        dir.join("main.kiro"),
        "import io\n\nio.print(\"manifest interpreted\")\n",
    )
    .expect("main module should be written");

    let output = run_kiro(&["interpret"], &dir);

    assert_success(&output, "interpret should resolve manifest entry");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("manifest interpreted"),
        "stdout should include manifest entry output:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn interpret_rejects_static_errors_before_execution() {
    let dir = temp_project("interpret_analysis");
    fs::write(
        dir.join("main.kiro"),
        r#"
import io

fn bad() {
    io.print(missing_name)
}

io.print("must not run")
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(&["interpret", "main.kiro"], &dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "invalid source should fail");
    assert!(
        stderr.contains("[KIRO2004:compile] Unknown variable 'missing_name'."),
        "stderr should show analyzer diagnostic:\n{}",
        stderr
    );
    assert!(
        !stdout.contains("must not run"),
        "interpreter should not execute after analyzer failure:\n{}",
        stdout
    );
}

#[test]
fn run_rejects_static_errors_before_execution_or_rust_build() {
    let dir = temp_project("run_analysis");
    link_runtime(&dir);
    fs::write(
        dir.join("main.kiro"),
        r#"
import io

fn bad() {
    io.print(missing_name)
}

io.print("must not run")
"#,
    )
    .expect("main module should be written");

    let output = run_kiro(&["run", "main.kiro"], &dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "invalid source should fail");
    assert!(
        stderr.contains("[KIRO2004:compile] Unknown variable 'missing_name'."),
        "stderr should show analyzer diagnostic:\n{}",
        stderr
    );
    assert!(
        !stdout.contains("must not run") && !stdout.contains("--- COMPILING ---"),
        "compiled run should stop at analyzer failure:\n{}",
        stdout
    );
    assert!(
        !stderr.contains("error[E"),
        "ordinary analyzer errors should not leak Rust diagnostics:\n{}",
        stderr
    );
}
