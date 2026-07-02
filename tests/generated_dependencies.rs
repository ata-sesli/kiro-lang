use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static KIRO_BUILD_LOCK: Mutex<()> = Mutex::new(());

fn temp_project(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "kiro_generated_dependencies_{}_{}_{}",
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

fn build_source(name: &str, source: &str) -> (String, String) {
    let _guard = KIRO_BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_project(name);
    link_runtime(&dir);
    fs::write(dir.join("main.kiro"), source).expect("source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
        .args(["build", "main.kiro"])
        .current_dir(&dir)
        .output()
        .expect("kiro-lang build should run");

    assert!(
        output.status.success(),
        "build should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let cargo_toml = fs::read_to_string(dir.join("kiro_build_cache/Cargo.toml"))
        .expect("generated Cargo.toml should exist");
    let main_rs = fs::read_to_string(dir.join("kiro_build_cache/src/main.rs"))
        .expect("generated main.rs should exist");

    (cargo_toml, main_rs)
}

fn assert_tokio_features(cargo_toml: &str, expected: &[&str], unexpected: &[&str]) {
    let tokio_line = cargo_toml
        .lines()
        .find(|line| line.trim_start().starts_with("tokio ="))
        .expect("generated Cargo.toml should contain tokio");

    for feature in expected {
        assert!(
            tokio_line.contains(&format!(r#""{}""#, feature)),
            "tokio feature '{}' should be present in:\n{}",
            feature,
            tokio_line
        );
    }

    for feature in unexpected {
        assert!(
            !tokio_line.contains(&format!(r#""{}""#, feature)),
            "tokio feature '{}' should be absent from:\n{}",
            feature,
            tokio_line
        );
    }
}

fn root_manifest() -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("root Cargo.toml should be readable")
}

fn root_dependency_line<'a>(manifest: &'a str, name: &str) -> Option<&'a str> {
    manifest.lines().find(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with(&format!("{} =", name))
    })
}

#[test]
fn root_manifest_keeps_future_deps_and_removes_generated_runtime_deps() {
    let manifest = root_manifest();

    assert!(
        root_dependency_line(&manifest, "rayon").is_some(),
        "rayon should remain available for future root work:\n{}",
        manifest
    );
    assert!(
        root_dependency_line(&manifest, "miette").is_some(),
        "miette should remain available for future root diagnostics:\n{}",
        manifest
    );
    for removed in [
        "anyhow",
        "async-channel",
        "reqwest",
        "rust-embed",
        "tree-sitter",
        "serde",
        "tokio",
    ] {
        assert!(
            root_dependency_line(&manifest, removed).is_none(),
            "{} should not be a root dependency:\n{}",
            removed,
            manifest
        );
    }

    assert!(
        manifest.contains("[features]"),
        "root manifest should define features:\n{}",
        manifest
    );
    assert!(
        manifest.contains(r#"default = ["lsp"]"#),
        "lsp should be enabled by default:\n{}",
        manifest
    );
    assert!(
        manifest
            .contains(r#"lsp = ["dep:lsp-server", "dep:lsp-types", "dep:serde_json", "dep:url"]"#),
        "lsp feature should own LSP dependencies:\n{}",
        manifest
    );
    for dependency in ["lsp-server", "lsp-types", "serde_json", "url"] {
        let line = root_dependency_line(&manifest, dependency).unwrap_or_else(|| {
            panic!("{} should be present as an optional dependency", dependency)
        });
        assert!(
            line.contains("optional = true"),
            "{} should be optional:\n{}",
            dependency,
            line
        );
    }
}

#[test]
fn unchanged_generated_files_are_not_rewritten_on_second_build() {
    let _guard = KIRO_BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_project("unchanged_mtimes");
    link_runtime(&dir);
    fs::write(dir.join("main.kiro"), "import io\n\nio.print(\"stable\")\n")
        .expect("source should be written");

    for build_number in 1..=2 {
        let output = Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
            .args(["build", "main.kiro"])
            .current_dir(&dir)
            .output()
            .expect("kiro-lang build should run");
        assert!(
            output.status.success(),
            "build {} should succeed\nstdout:\n{}\nstderr:\n{}",
            build_number,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        if build_number == 1 {
            std::thread::sleep(Duration::from_millis(1100));
        }
    }

    let generated = [
        dir.join("kiro_build_cache/Cargo.toml"),
        dir.join("kiro_build_cache/src/main.rs"),
        dir.join("kiro_build_cache/src/header.rs"),
    ];
    let first_mtimes: Vec<SystemTime> = generated
        .iter()
        .map(|path| {
            fs::metadata(path)
                .unwrap_or_else(|e| panic!("metadata should exist for {}: {}", path.display(), e))
                .modified()
                .expect("modified time should be available")
        })
        .collect();

    std::thread::sleep(Duration::from_millis(1100));

    let output = Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
        .args(["build", "main.kiro"])
        .current_dir(&dir)
        .output()
        .expect("kiro-lang build should run");
    assert!(
        output.status.success(),
        "third build should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    for (idx, path) in generated.iter().enumerate() {
        let after = fs::metadata(path)
            .unwrap_or_else(|e| panic!("metadata should exist for {}: {}", path.display(), e))
            .modified()
            .expect("modified time should be available");
        assert_eq!(
            first_mtimes[idx],
            after,
            "unchanged generated file should not be rewritten: {}",
            path.display()
        );
    }
}

#[test]
fn plain_script_uses_minimal_async_dependencies() {
    let (cargo_toml, main_rs) = build_source(
        "plain",
        r#"import io

io.print("hello")"#,
    );

    assert_tokio_features(
        &cargo_toml,
        &["macros", "rt-multi-thread"],
        &["fs", "time", "sync"],
    );
    assert!(
        !cargo_toml.contains("reqwest"),
        "plain script should not depend on reqwest:\n{}",
        cargo_toml
    );
    assert!(
        !cargo_toml.contains("async-channel"),
        "plain script should not depend on async-channel:\n{}",
        cargo_toml
    );
    assert!(
        !cargo_toml.contains("anyhow"),
        "plain script should not depend on anyhow:\n{}",
        cargo_toml
    );
    assert!(
        !main_rs.contains("use async_channel;"),
        "plain script should not import async_channel:\n{}",
        main_rs
    );
    assert!(
        !main_rs.contains("struct KiroPipe"),
        "plain script should not define KiroPipe:\n{}",
        main_rs
    );
}

#[test]
fn generated_anyhow_is_kept_when_error_values_need_it() {
    let (cargo_toml, main_rs) = build_source(
        "error_def",
        r#"
import io

error Bad = "bad"

io.print("ok")
"#,
    );

    assert!(
        cargo_toml.contains("anyhow"),
        "error definitions still need anyhow in generated dependencies:\n{}",
        cargo_toml
    );
    assert!(
        main_rs.contains("anyhow::"),
        "error definitions should still generate anyhow usage:\n{}",
        main_rs
    );
}

#[test]
fn std_net_header_reuses_one_reqwest_client() {
    let header = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/kiro_std/net/header.rs"),
    )
    .expect("std_net header should be readable");

    assert!(
        header.contains("OnceLock<reqwest::Client>"),
        "std_net should keep one reusable reqwest client:\n{}",
        header
    );
    assert!(
        !header.contains("reqwest::get("),
        "std_net should route get/status through the shared client:\n{}",
        header
    );
}

#[test]
fn generated_main_uses_runtime_helpers_instead_of_inlining_them() {
    let (_, main_rs) = build_source(
        "runtime_helpers",
        r#"import io

io.print("hello")"#,
    );

    assert!(
        main_rs.contains("use kiro_runtime::*;"),
        "generated main.rs should import shared runtime helpers:\n{}",
        main_rs
    );
    for helper in [
        "pub type KiroResult",
        "pub trait KiroAdd",
        "pub trait KiroEq",
        "pub struct KiroAdrVoid",
    ] {
        assert!(
            !main_rs.contains(helper),
            "generated main.rs should not inline '{}':\n{}",
            helper,
            main_rs
        );
    }
}

#[test]
fn std_fs_import_enables_tokio_fs_only() {
    let (cargo_toml, _) = build_source(
        "std_fs",
        r#"
import io

import std_fs

fn main() {
    var exists = std_fs.exists("missing.txt")
    io.print(exists)
}

main()
"#,
    );

    assert_tokio_features(
        &cargo_toml,
        &["macros", "rt-multi-thread", "fs"],
        &["time", "sync"],
    );
    assert!(
        !cargo_toml.contains("reqwest"),
        "std_fs script should not depend on reqwest:\n{}",
        cargo_toml
    );
}

#[test]
fn std_time_import_enables_tokio_time_only() {
    let (cargo_toml, _) = build_source(
        "std_time",
        r#"
import io

import std_time

fn main() {
    io.print(std_time.now())
}

main()
"#,
    );

    assert_tokio_features(
        &cargo_toml,
        &["macros", "rt-multi-thread", "time"],
        &["fs", "sync"],
    );
    assert!(
        !cargo_toml.contains("reqwest"),
        "std_time script should not depend on reqwest:\n{}",
        cargo_toml
    );
}

#[test]
fn std_net_import_enables_reqwest_without_direct_fs_or_time_features() {
    let (cargo_toml, _) = build_source(
        "std_net",
        r#"
import io

import std_net

fn main() {
    io.print(std_net.body("cached"))
}

main()
"#,
    );

    assert_tokio_features(
        &cargo_toml,
        &["macros", "rt-multi-thread"],
        &["fs", "time", "sync"],
    );
    assert!(
        cargo_toml.contains("reqwest"),
        "std_net script should depend on reqwest:\n{}",
        cargo_toml
    );
}

#[test]
fn pipe_operations_enable_async_channel_and_kiro_pipe() {
    let (cargo_toml, main_rs) = build_source(
        "pipe_ops",
        r#"
import io

fn main() {
    var ch = pipe num
    give ch 7
    io.print(take ch)
}

main()
"#,
    );

    assert!(
        cargo_toml.contains("async-channel"),
        "pipe script should depend on async-channel:\n{}",
        cargo_toml
    );
    assert!(
        main_rs.contains("use async_channel;"),
        "pipe script should import async_channel:\n{}",
        main_rs
    );
    assert!(
        main_rs.contains("struct KiroPipe"),
        "pipe script should define KiroPipe:\n{}",
        main_rs
    );
}

#[test]
fn io_alias_print_accepts_displayable_values() {
    let (cargo_toml, main_rs) = build_source(
        "io_alias_print",
        r#"
import io

io.print(42)
io.print(true)
io.write("done")
"#,
    );

    assert!(
        cargo_toml.contains("tokio"),
        "generated Cargo.toml should still contain tokio for async main:\n{}",
        cargo_toml
    );
    assert!(
        main_rs.contains("println!(\"{}\", (42.0).clone());"),
        "io.print should lower to stdout newline display:\n{}",
        main_rs
    );
    assert!(
        main_rs.contains("print!(\"{}\", (String::from(\"done\")).clone());"),
        "io.write should lower to stdout display without newline:\n{}",
        main_rs
    );
}

#[test]
fn pipe_type_only_enables_async_channel_and_kiro_pipe() {
    let (cargo_toml, main_rs) = build_source(
        "pipe_type",
        r#"
import io

fn worker(ch: pipe num) {
    io.print("ok")
}

fn main() {
    io.print("ready")
}

main()
"#,
    );

    assert!(
        cargo_toml.contains("async-channel"),
        "pipe type should depend on async-channel:\n{}",
        cargo_toml
    );
    assert!(
        main_rs.contains("KiroPipe<f64>"),
        "pipe type should compile to KiroPipe<f64>:\n{}",
        main_rs
    );
}
