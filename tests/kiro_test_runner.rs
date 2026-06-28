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
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "kiro_test_runner_{}_{}_{}",
        name,
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&dir).expect("temp project should be created");
    dir
}

fn run_kiro(args: &[&str], current_dir: &Path) -> std::process::Output {
    let _guard = KIRO_BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
        .args(args)
        .current_dir(current_dir)
        .output()
        .expect("kiro-lang command should run")
}

fn write_manifest(dir: &Path) {
    fs::write(
        dir.join("kiro.toml"),
        r#"[package]
name = "test_app"
entry = "main.kiro"

[dependencies]
"#,
    )
    .expect("manifest should be written");
    fs::write(dir.join("main.kiro"), "print \"main\"\n").expect("entry should be written");
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

#[test]
fn test_command_discovers_project_tests_from_parent_root() {
    let dir = temp_project("discover_parent");
    link_runtime(&dir);
    write_manifest(&dir);
    let nested = dir.join("tasks/nested");
    fs::create_dir_all(&nested).expect("nested dir should be created");
    fs::write(dir.join("math_test.kiro"), "check 2 + 3 == 5, \"math\"\n")
        .expect("test should be written");
    fs::write(dir.join("helper.kiro"), "check false, \"should not run\"\n")
        .expect("non-test file should be written");
    fs::create_dir_all(dir.join("target")).expect("target dir should be created");
    fs::write(
        dir.join("target/ignored_test.kiro"),
        "check false, \"target\"\n",
    )
    .expect("ignored target test should be written");
    fs::create_dir_all(dir.join(".hidden")).expect("hidden dir should be created");
    fs::write(
        dir.join(".hidden/ignored_test.kiro"),
        "check false, \"hidden\"\n",
    )
    .expect("ignored hidden test should be written");

    let output = run_kiro(&["test"], &nested);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "kiro test should pass\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("PASS"),
        "stdout should show pass:\n{}",
        stdout
    );
    assert!(
        stdout.contains("math_test.kiro"),
        "stdout should include discovered test:\n{}",
        stdout
    );
    assert!(
        stdout.contains("test result: ok. 1 passed; 0 failed."),
        "stdout should include summary:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("helper.kiro")
            && !stdout.contains("ignored_test.kiro")
            && !stderr.contains("should not run"),
        "non-test or skipped files should not run\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
}

#[test]
fn explicit_file_runs_even_without_test_suffix() {
    let dir = temp_project("explicit_file");
    link_runtime(&dir);
    fs::write(dir.join("contract.kiro"), "check true, \"contract\"\n")
        .expect("explicit test should be written");

    let output = run_kiro(&["test", "contract.kiro"], &dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "explicit file should pass\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("PASS contract.kiro"),
        "stdout should include explicit file pass:\n{}",
        stdout
    );
}

#[test]
fn failing_check_marks_file_failed_and_runner_continues() {
    let dir = temp_project("failing_check");
    link_runtime(&dir);
    fs::write(dir.join("a_fail_test.kiro"), "check false, \"boom\"\n")
        .expect("failing test should be written");
    fs::write(dir.join("b_pass_test.kiro"), "check true, \"ok\"\n")
        .expect("passing test should be written");

    let output = run_kiro(&["test"], &dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "failing suite should fail");
    assert!(
        stdout.contains("FAIL a_fail_test.kiro") && stdout.contains("PASS b_pass_test.kiro"),
        "runner should report fail and continue\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("[KIRO3001:runtime] Check failed: boom"),
        "failure output should include Kiro check diagnostic\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("test result: FAILED. 1 passed; 1 failed."),
        "stdout should include failure summary:\n{}",
        stdout
    );
}

#[test]
fn compile_error_marks_file_failed_and_runner_continues() {
    let dir = temp_project("compile_error");
    link_runtime(&dir);
    fs::write(dir.join("a_bad_test.kiro"), "print missing_name\n")
        .expect("bad test should be written");
    fs::write(dir.join("b_good_test.kiro"), "check true, \"ok\"\n")
        .expect("good test should be written");

    let output = run_kiro(&["test"], &dir);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "compile failure should fail suite"
    );
    assert!(
        stdout.contains("FAIL a_bad_test.kiro") && stdout.contains("PASS b_good_test.kiro"),
        "runner should continue after compile failure:\n{}",
        stdout
    );
    assert!(
        stdout.contains("[KIRO2004:compile] Unknown variable 'missing_name'."),
        "stdout should include Kiro compile diagnostic:\n{}",
        stdout
    );
}

#[test]
fn directory_paths_discover_only_test_files_and_import_sibling_modules() {
    let dir = temp_project("directory_path");
    link_runtime(&dir);
    let tests_dir = dir.join("tests");
    fs::create_dir_all(&tests_dir).expect("tests dir should be created");
    fs::write(
        tests_dir.join("math.kiro"),
        "pure fn add(a: num, b: num) -> num { return a + b }\n",
    )
    .expect("sibling module should be written");
    fs::write(
        tests_dir.join("math_test.kiro"),
        "import math\ncheck math.add(2, 3) == 5, \"add\"\n",
    )
    .expect("test should be written");
    fs::write(tests_dir.join("ignored.kiro"), "check false, \"ignored\"\n")
        .expect("ignored file should be written");

    let output = run_kiro(&["test", "tests"], &dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "directory test should pass\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("PASS tests/math_test.kiro"),
        "stdout should include discovered directory test:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("ignored.kiro") && !stderr.contains("ignored"),
        "non-test file should not run\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
}

#[test]
fn no_tests_found_exits_successfully() {
    let dir = temp_project("no_tests");

    let output = run_kiro(&["test"], &dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "no tests should be success\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert_eq!(stdout.trim(), "No tests found.");
}
