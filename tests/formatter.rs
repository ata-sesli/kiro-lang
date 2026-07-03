use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use kiro_lang::formatter::format_source;

fn temp_project(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "kiro_formatter_{}_{}_{}",
        name,
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&dir).expect("temp project should be created");
    dir
}

fn run_kiro(args: &[&str], current_dir: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
        .args(args)
        .current_dir(current_dir)
        .output()
        .expect("kiro-lang command should run")
}

#[test]
fn format_source_fixes_indentation_and_basic_spacing() {
    let input = r#"
import io

fn worker(){
io.print("hello")
on (true){
rest
}
}
"#;

    let formatted = format_source(input).expect("source should format");

    assert_eq!(
        formatted,
        r#"import io

fn worker() {
    io.print("hello")
    on (true) {
        rest
    }
}
"#
    );
}

#[test]
fn format_source_preserves_comments_and_literal_shape() {
    let input = r#"
/// Builds a user.
fn worker(){
// keep this note
var user=User { name:"Kiro", active:true } // inline comment
var nums=list num {
1,
2
}
}
"#;

    let formatted = format_source(input).expect("source should format");

    assert_eq!(
        formatted,
        r#"/// Builds a user.
fn worker() {
    // keep this note
    var user = User { name: "Kiro", active: true } // inline comment
    var nums = list num {
        1,
        2
    }
}
"#
    );
}

#[test]
fn format_source_normalizes_top_level_blank_lines_and_is_idempotent() {
    let input = r#"
import io



fn first(){
io.print("one")
}


fn second(){

io.print("two")


}
"#;

    let formatted = format_source(input).expect("source should format");
    let formatted_again = format_source(&formatted).expect("formatted source should format again");

    assert_eq!(formatted, formatted_again);
    assert_eq!(
        formatted,
        r#"import io

fn first() {
    io.print("one")
}

fn second() {

    io.print("two")

}
"#
    );
}

#[test]
fn format_source_separates_documented_top_level_items() {
    let input = r#"
fn first() {
}
/// Documents second.
fn second() {
}
"#;

    let formatted = format_source(input).expect("source should format");

    assert_eq!(
        formatted,
        r#"fn first() {
}

/// Documents second.
fn second() {
}
"#
    );
}

#[test]
fn format_source_keeps_ranges_and_failable_returns_tight() {
    let input = r#"
import io

rust fn read(path:str)->str!

fn main(){
loop x in 1..4{
io.print(x)
}
}
"#;

    let formatted = format_source(input).expect("source should format");

    assert_eq!(
        formatted,
        r#"import io

rust fn read(path: str) -> str!

fn main() {
    loop x in 1..4 {
        io.print(x)
    }
}
"#
    );
}

#[test]
fn cli_fmt_rewrites_file_in_place() {
    let dir = temp_project("rewrite");
    let file = dir.join("main.kiro");
    fs::write(&file, "import io\n\nfn main(){\nio.print(\"hi\")\n}\n")
        .expect("source should be written");

    let output = run_kiro(&["fmt", file.to_str().unwrap()], &dir);

    assert!(
        output.status.success(),
        "fmt should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&file).expect("formatted source should be readable"),
        "import io\n\nfn main() {\n    io.print(\"hi\")\n}\n"
    );
}

#[test]
fn cli_fmt_check_reports_unformatted_files_without_writing() {
    let dir = temp_project("check_dirty");
    let file = dir.join("main.kiro");
    fs::write(&file, "import io\n\nfn main(){\nio.print(\"hi\")\n}\n")
        .expect("source should be written");

    let output = run_kiro(&["fmt", "--check", file.to_str().unwrap()], &dir);

    assert!(
        !output.status.success(),
        "check should fail for unformatted source"
    );
    assert_eq!(
        fs::read_to_string(&file).expect("source should be unchanged"),
        "import io\n\nfn main(){\nio.print(\"hi\")\n}\n"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Would format"),
        "stdout should list unformatted files:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn cli_fmt_check_passes_after_formatting() {
    let dir = temp_project("check_clean");
    let file = dir.join("main.kiro");
    fs::write(&file, "import io\n\nfn main(){\nio.print(\"hi\")\n}\n")
        .expect("source should be written");

    let write_output = run_kiro(&["fmt", file.to_str().unwrap()], &dir);
    assert!(write_output.status.success(), "initial fmt should succeed");

    let check_output = run_kiro(&["fmt", "--check", file.to_str().unwrap()], &dir);
    assert!(
        check_output.status.success(),
        "check should pass after formatting\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check_output.stdout),
        String::from_utf8_lossy(&check_output.stderr)
    );
}

#[test]
fn cli_fmt_discovers_project_files_and_skips_cache_dirs() {
    let dir = temp_project("discover");
    let main = dir.join("main.kiro");
    let nested_dir = dir.join("scripts");
    let cache_dir = dir.join(".kiro/build");
    fs::create_dir_all(&nested_dir).expect("nested dir should be created");
    fs::create_dir_all(&cache_dir).expect("cache dir should be created");
    fs::write(&main, "import io\n\nfn main(){\nio.print(\"main\")\n}\n")
        .expect("main source should be written");
    fs::write(nested_dir.join("task.kiro"), "fn task(){\nrest\n}\n")
        .expect("nested source should be written");
    fs::write(
        cache_dir.join("generated.kiro"),
        "fn generated(){\nrest\n}\n",
    )
    .expect("cache source should be written");

    let output = run_kiro(&["fmt"], &dir);

    assert!(
        output.status.success(),
        "fmt discovery should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&main).expect("main source should be readable"),
        "import io\n\nfn main() {\n    io.print(\"main\")\n}\n"
    );
    assert_eq!(
        fs::read_to_string(nested_dir.join("task.kiro")).expect("nested source should be readable"),
        "fn task() {\n    rest\n}\n"
    );
    assert_eq!(
        fs::read_to_string(cache_dir.join("generated.kiro"))
            .expect("cache source should be readable"),
        "fn generated(){\nrest\n}\n"
    );
}

#[test]
fn cli_fmt_invalid_source_reports_parse_error_without_writing() {
    let dir = temp_project("invalid");
    let file = dir.join("main.kiro");
    fs::write(&file, "import io\n\nfn main( {\nio.print(\"hi\")\n}\n")
        .expect("source should be written");

    let output = run_kiro(&["fmt", file.to_str().unwrap()], &dir);

    assert!(
        !output.status.success(),
        "invalid source should fail formatting"
    );
    assert_eq!(
        fs::read_to_string(&file).expect("invalid source should be unchanged"),
        "import io\n\nfn main( {\nio.print(\"hi\")\n}\n"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("[KIRO1003:parse]"),
        "stderr should contain Kiro parse diagnostic:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("main.kiro:3:"),
        "stderr should contain parse source location:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("3 | fn main( {"),
        "stderr should contain invalid source line:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_fmt_removed_print_statement_reports_migration_diagnostic_without_writing() {
    let dir = temp_project("removed_print_statement");
    let file = dir.join("main.kiro");
    fs::write(&file, "print \"hi\"\n").expect("source should be written");

    let output = run_kiro(&["fmt", file.to_str().unwrap()], &dir);

    assert!(
        !output.status.success(),
        "removed print statement should fail formatting"
    );
    assert_eq!(
        fs::read_to_string(&file).expect("source should be unchanged"),
        "print \"hi\"\n"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[KIRO1002:parse] 'print' statement was removed."),
        "stderr should contain Kiro migration diagnostic:\n{}",
        stderr
    );
    assert!(
        stderr.contains("help: use `import io` and `io.print(value)`"),
        "stderr should contain migration help:\n{}",
        stderr
    );
}
