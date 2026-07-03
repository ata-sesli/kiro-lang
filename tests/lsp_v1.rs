#![cfg(feature = "lsp")]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use url::Url;

fn temp_project(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "kiro_lsp_v1_{}_{}_{}",
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

fn file_uri(path: &Path) -> String {
    Url::from_file_path(path)
        .expect("path should convert to file uri")
        .to_string()
}

fn start_lsp(current_dir: &Path) -> (Child, ChildStdin, BufReader<ChildStdout>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_kiro-lang"))
        .arg("lsp")
        .current_dir(current_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("kiro lsp should start");
    let stdin = child.stdin.take().expect("stdin should be piped");
    let stdout = BufReader::new(child.stdout.take().expect("stdout should be piped"));
    (child, stdin, stdout)
}

fn send_lsp(stdin: &mut ChildStdin, message: Value) {
    let body = message.to_string();
    write!(stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body)
        .expect("lsp message should be written");
    stdin.flush().expect("lsp stdin should flush");
}

fn read_lsp(stdout: &mut BufReader<ChildStdout>) -> Value {
    let mut content_len = None;
    loop {
        let mut line = String::new();
        let bytes = stdout.read_line(&mut line).expect("lsp header should read");
        assert!(bytes > 0, "lsp server closed stdout");
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(raw) = trimmed.strip_prefix("Content-Length:") {
            content_len = Some(
                raw.trim()
                    .parse::<usize>()
                    .expect("content length should parse"),
            );
        }
    }
    let len = content_len.expect("lsp message should include content length");
    let mut body = vec![0_u8; len];
    stdout
        .read_exact(&mut body)
        .expect("lsp body should read exactly");
    serde_json::from_slice(&body).expect("lsp body should be json")
}

fn read_response(stdout: &mut BufReader<ChildStdout>, id: i64) -> Value {
    loop {
        let message = read_lsp(stdout);
        if message.get("id").and_then(Value::as_i64) == Some(id) {
            return message;
        }
    }
}

fn read_notification(stdout: &mut BufReader<ChildStdout>, method: &str) -> Value {
    loop {
        let message = read_lsp(stdout);
        if message.get("method").and_then(Value::as_str) == Some(method) {
            return message;
        }
    }
}

fn initialize(stdin: &mut ChildStdin, stdout: &mut BufReader<ChildStdout>, root: &Path) {
    send_lsp(
        stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": Url::from_directory_path(root).unwrap().to_string(),
                "capabilities": {}
            }
        }),
    );
    let response = read_response(stdout, 1);
    let capabilities = &response["result"]["capabilities"];
    assert_eq!(capabilities["documentFormattingProvider"], true);
    assert_eq!(capabilities["definitionProvider"], true);
    assert!(capabilities.get("hoverProvider").is_some());
    assert!(capabilities.get("completionProvider").is_some());
    assert!(capabilities.get("documentSymbolProvider").is_some());
    assert!(capabilities.get("signatureHelpProvider").is_some());

    send_lsp(
        stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }),
    );
}

fn shutdown(mut child: Child, stdin: &mut ChildStdin, stdout: &mut BufReader<ChildStdout>) {
    send_lsp(
        stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 99,
            "method": "shutdown",
            "params": null
        }),
    );
    let _ = read_response(stdout, 99);
    send_lsp(
        stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "exit"
        }),
    );
    let _ = child.wait();
}

#[test]
fn static_check_validates_without_running_program() {
    let dir = temp_project("static_check");
    let file = dir.join("main.kiro");
    fs::write(
        &file,
        "import io\n\nio.print(\"should not run during check\")\n",
    )
    .expect("file should be written");

    let output = run_kiro(&["check", file.to_str().unwrap()], &dir);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "static check should pass\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("OK"),
        "stdout should confirm OK:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("should not run during check"),
        "check must not execute program output:\n{}",
        stdout
    );
}

#[test]
fn static_check_catches_errors_in_uncalled_functions_and_unreached_branches() {
    let dir = temp_project("static_errors");
    let uncalled = dir.join("uncalled.kiro");
    fs::write(
        &uncalled,
        r#"
import io

fn bad() {
    io.print(missing_name)
}
"#,
    )
    .expect("uncalled file should be written");

    let output = run_kiro(&["check", uncalled.to_str().unwrap()], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "uncalled error should fail");
    assert!(
        stderr.contains("[KIRO2004:compile] Unknown variable 'missing_name'."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "should not leak Rust:\n{}",
        stderr
    );

    let unreachable = dir.join("unreachable.kiro");
    fs::write(
        &unreachable,
        r#"
on (false) {
    missing()
}
"#,
    )
    .expect("unreachable file should be written");

    let output = run_kiro(&["check", unreachable.to_str().unwrap()], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "unreached error should fail");
    assert!(
        stderr.contains("[KIRO2004:compile] Unknown function 'missing'."),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn static_check_collects_import_metadata_and_missing_glue() {
    let dir = temp_project("metadata_glue");
    fs::write(
        dir.join("main.kiro"),
        r#"
import io

import math

io.print(math.add(1))
"#,
    )
    .expect("main should be written");
    fs::write(
        dir.join("math.kiro"),
        r#"
pure fn add(a: num, b: num) -> num {
    return a + b
}
"#,
    )
    .expect("math should be written");

    let output = run_kiro(&["check", "main.kiro"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "bad imported call should fail");
    assert!(
        stderr.contains("Wrong argument count for 'math.add': expected 2, got 1."),
        "unexpected stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("error[E"),
        "should not leak Rust:\n{}",
        stderr
    );

    fs::write(
        dir.join("host.kiro"),
        "rust fn read_file(path: str) -> str!\n",
    )
    .expect("host should be written");
    let output = run_kiro(&["check", "host.kiro"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "missing glue should fail");
    assert!(
        stderr.contains("[KIRO2009:compile] Missing Rust glue for host function 'host.read_file'."),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn lsp_publishes_diagnostics_only_after_save_and_clears_after_fix() {
    let dir = temp_project("diagnostics");
    let file = dir.join("main.kiro");
    fs::write(&file, "import io\n\nio.print(\"initial\")\n").expect("file should be written");
    let uri = file_uri(&file);

    let (child, mut stdin, mut stdout) = start_lsp(&dir);
    initialize(&mut stdin, &mut stdout, &dir);

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "kiro",
                    "version": 1,
                    "text": "import io\n\nio.print(missing_name)\n"
                }
            }
        }),
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": {
                "textDocument": { "uri": uri },
                "text": "import io\n\nio.print(missing_name)\n"
            }
        }),
    );
    let published = read_notification(&mut stdout, "textDocument/publishDiagnostics");
    let diagnostics = published["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics should be array");
    assert_eq!(diagnostics.len(), 1, "unexpected publish: {}", published);
    assert_eq!(diagnostics[0]["code"], "KIRO2004");
    assert!(
        diagnostics[0]["message"]
            .as_str()
            .unwrap()
            .contains("Unknown variable 'missing_name'.")
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": [{ "text": "import io\n\nio.print(\"fixed\")\n" }]
            }
        }),
    );
    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": {
                "textDocument": { "uri": uri },
                "text": "import io\n\nio.print(\"fixed\")\n"
            }
        }),
    );
    let published = read_notification(&mut stdout, "textDocument/publishDiagnostics");
    assert_eq!(
        published["params"]["diagnostics"].as_array().unwrap().len(),
        0,
        "fixed document should clear diagnostics: {}",
        published
    );

    shutdown(child, &mut stdin, &mut stdout);
}

#[test]
fn lsp_reports_removed_print_statement_on_save() {
    let dir = temp_project("removed_print_diagnostic");
    let file = dir.join("main.kiro");
    fs::write(&file, "print \"initial\"\n").expect("file should be written");
    let uri = file_uri(&file);

    let (child, mut stdin, mut stdout) = start_lsp(&dir);
    initialize(&mut stdin, &mut stdout, &dir);

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "kiro",
                    "version": 1,
                    "text": "print \"old\"\n"
                }
            }
        }),
    );
    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": {
                "textDocument": { "uri": uri },
                "text": "print \"old\"\n"
            }
        }),
    );

    let published = read_notification(&mut stdout, "textDocument/publishDiagnostics");
    let diagnostics = published["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics should be array");
    assert_eq!(diagnostics.len(), 1, "unexpected publish: {}", published);
    assert_eq!(diagnostics[0]["code"], "KIRO1002");
    assert!(
        diagnostics[0]["message"]
            .as_str()
            .unwrap()
            .contains("'print' statement was removed.")
    );
    assert!(
        diagnostics[0]["message"]
            .as_str()
            .unwrap()
            .contains("use `import io` and `io.print(value)`")
    );

    shutdown(child, &mut stdin, &mut stdout);
}

#[test]
fn lsp_format_hover_completion_and_symbols_work() {
    let dir = temp_project("features");
    let file = dir.join("main.kiro");
    fs::write(
        &file,
        r#"
import io

import math

fn worker() {
io.print("hi")
}

worker()
math.add(1, 2)
"#,
    )
    .expect("file should be written");
    fs::write(
        dir.join("math.kiro"),
        r#"
pure fn add(a: num, b: num) -> num {
    return a + b
}
"#,
    )
    .expect("math should be written");
    let uri = file_uri(&file);

    let (child, mut stdin, mut stdout) = start_lsp(&dir);
    initialize(&mut stdin, &mut stdout, &dir);

    let source = fs::read_to_string(&file).expect("source should read");
    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "kiro",
                    "version": 1,
                    "text": source
                }
            }
        }),
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/formatting",
            "params": {
                "textDocument": { "uri": uri },
                "options": { "tabSize": 4, "insertSpaces": true }
            }
        }),
    );
    let formatting = read_response(&mut stdout, 2);
    let edits = formatting["result"]
        .as_array()
        .expect("edits should be array");
    assert_eq!(edits.len(), 1, "formatting should return one edit");
    assert!(
        edits[0]["newText"]
            .as_str()
            .unwrap()
            .contains("    io.print(\"hi\")"),
        "formatted text should indent function body: {}",
        formatting
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 5, "character": 0 }
            }
        }),
    );
    let hover = read_response(&mut stdout, 3);
    assert!(
        hover["result"]["contents"]
            .to_string()
            .contains("Defines a function"),
        "hover should describe fn: {}",
        hover
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 7, "character": 0 }
            }
        }),
    );
    let completion = read_response(&mut stdout, 4);
    let labels = completion["result"]
        .as_array()
        .expect("completion should be array")
        .iter()
        .filter_map(|item| item["label"].as_str())
        .collect::<Vec<_>>();
    assert!(
        labels.contains(&"fn"),
        "keyword completion missing: {:?}",
        labels
    );
    assert!(
        labels.contains(&"handle"),
        "handle keyword completion missing: {:?}",
        labels
    );
    assert!(
        labels.contains(&"worker"),
        "local function completion missing: {:?}",
        labels
    );
    assert!(
        !labels.contains(&"print"),
        "removed print keyword should not be completed: {:?}",
        labels
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 10, "character": 5 }
            }
        }),
    );
    let module_completion = read_response(&mut stdout, 5);
    let labels = module_completion["result"]
        .as_array()
        .expect("module completion should be array")
        .iter()
        .filter_map(|item| item["label"].as_str())
        .collect::<Vec<_>>();
    assert!(
        labels.contains(&"add"),
        "module function completion missing: {:?}",
        labels
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "textDocument/documentSymbol",
            "params": {
                "textDocument": { "uri": uri }
            }
        }),
    );
    let symbols = read_response(&mut stdout, 6);
    let names = symbols["result"]
        .as_array()
        .expect("symbols should be array")
        .iter()
        .filter_map(|item| item["name"].as_str())
        .collect::<Vec<_>>();
    assert!(
        names.contains(&"math"),
        "import symbol missing: {:?}",
        names
    );
    assert!(
        names.contains(&"worker"),
        "function symbol missing: {:?}",
        names
    );

    shutdown(child, &mut stdin, &mut stdout);
}

#[test]
fn lsp_symbol_intelligence_handles_definition_hover_signature_and_symbols() {
    let dir = temp_project("symbol_intelligence");
    let main = dir.join("main.kiro");
    let math = dir.join("math.kiro");
    fs::write(
        &main,
        r#"import math

/// Does work.
fn worker(value: num) -> num {
    return math.add(value, 1)
}

struct User { name: str age: num }
error NotFound = "missing"
rust fn host_read(path: str) -> str!

var total = math.add(1, 2)

worker(total)
math.add(1, 2)
"#,
    )
    .expect("main file should be written");
    fs::write(
        &math,
        r#"pure fn seed() -> num { return 0 }

/// Adds numbers.
pure fn add(a: num, b: num) -> num {
    return a + b
}
"#,
    )
    .expect("math file should be written");
    fs::write(
        dir.join("main.rs"),
        r#"pub async fn host_read(_args: Vec<kiro_runtime::RuntimeVal>) -> kiro_runtime::HostResult {
    Ok("ok".into())
}
"#,
    )
    .expect("host glue should be written");

    let main_uri = file_uri(&main);
    let math_uri = file_uri(&math);
    let (child, mut stdin, mut stdout) = start_lsp(&dir);
    initialize(&mut stdin, &mut stdout, &dir);
    let source = fs::read_to_string(&main).expect("main source should read");
    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": main_uri,
                    "languageId": "kiro",
                    "version": 1,
                    "text": source
                }
            }
        }),
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 20,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": main_uri },
                "position": { "line": 13, "character": 1 }
            }
        }),
    );
    let definition = read_response(&mut stdout, 20);
    assert_eq!(definition["result"]["uri"], main_uri);
    assert_eq!(definition["result"]["range"]["start"]["line"], 3);

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": main_uri },
                "position": { "line": 14, "character": 6 }
            }
        }),
    );
    let definition = read_response(&mut stdout, 21);
    assert_eq!(definition["result"]["uri"], math_uri);
    assert_eq!(definition["result"]["range"]["start"]["line"], 3);

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 25,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": main_uri },
                "position": { "line": 14, "character": 1 }
            }
        }),
    );
    let definition = read_response(&mut stdout, 25);
    assert_eq!(definition["result"]["uri"], math_uri);
    assert_eq!(definition["result"]["range"]["start"]["line"], 0);

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 22,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": main_uri },
                "position": { "line": 13, "character": 1 }
            }
        }),
    );
    let hover = read_response(&mut stdout, 22);
    let hover_text = hover["result"]["contents"].to_string();
    assert!(
        hover_text.contains("fn worker(value: num) -> num"),
        "hover should include Kiro signature: {}",
        hover
    );
    assert!(
        hover_text.contains("Does work."),
        "hover should include doc comment: {}",
        hover
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 23,
            "method": "textDocument/signatureHelp",
            "params": {
                "textDocument": { "uri": main_uri },
                "position": { "line": 14, "character": 12 }
            }
        }),
    );
    let signature = read_response(&mut stdout, 23);
    assert_eq!(
        signature["result"]["signatures"][0]["label"],
        "pure fn add(a: num, b: num) -> num"
    );
    assert_eq!(signature["result"]["activeParameter"], 1);

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 24,
            "method": "textDocument/documentSymbol",
            "params": {
                "textDocument": { "uri": main_uri }
            }
        }),
    );
    let symbols = read_response(&mut stdout, 24);
    let items = symbols["result"]
        .as_array()
        .expect("symbols should be array");
    let names = items
        .iter()
        .filter_map(|item| item["name"].as_str())
        .collect::<Vec<_>>();
    assert!(
        names.contains(&"math"),
        "import symbol missing: {:?}",
        names
    );
    assert!(
        names.contains(&"worker"),
        "function symbol missing: {:?}",
        names
    );
    assert!(
        names.contains(&"User"),
        "struct symbol missing: {:?}",
        names
    );
    assert!(
        names.contains(&"NotFound"),
        "error symbol missing: {:?}",
        names
    );
    assert!(
        names.contains(&"host_read"),
        "rust fn symbol missing: {:?}",
        names
    );
    assert!(names.contains(&"total"), "var symbol missing: {:?}", names);
    let worker = items
        .iter()
        .find(|item| item["name"] == "worker")
        .expect("worker symbol should exist");
    assert_eq!(worker["range"]["start"]["line"], 3);
    assert_eq!(worker["selectionRange"]["start"]["character"], 3);

    for (name, line, character) in [
        ("math", 0, 7),
        ("User", 7, 7),
        ("NotFound", 8, 6),
        ("host_read", 9, 8),
        ("total", 11, 4),
    ] {
        let item = items
            .iter()
            .find(|item| item["name"] == name)
            .unwrap_or_else(|| panic!("{name} symbol should exist"));
        assert_eq!(
            item["selectionRange"]["start"]["line"], line,
            "{name} should start on exact declaration line"
        );
        assert_eq!(
            item["selectionRange"]["start"]["character"], character,
            "{name} should start on exact declaration name column"
        );
    }

    shutdown(child, &mut stdin, &mut stdout);
}

#[test]
fn lsp_knows_io_display_helpers() {
    let dir = temp_project("io_helpers");
    let main = dir.join("main.kiro");
    fs::write(
        &main,
        r#"import io

io.print(42)
io.
"#,
    )
    .expect("main file should be written");

    let uri = file_uri(&main);
    let (child, mut stdin, mut stdout) = start_lsp(&dir);
    initialize(&mut stdin, &mut stdout, &dir);
    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "kiro",
                    "version": 1,
                    "text": fs::read_to_string(&main).expect("source should read")
                }
            }
        }),
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 26,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 3, "character": 3 }
            }
        }),
    );
    let completion = read_response(&mut stdout, 26);
    let items = completion["result"]
        .as_array()
        .expect("completion should be array");
    let print_item = items
        .iter()
        .find(|item| item["label"] == "print")
        .expect("io.print completion should exist");
    assert_eq!(print_item["detail"], "io.print(value) -> void");

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 27,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 2, "character": 5 }
            }
        }),
    );
    let hover = read_response(&mut stdout, 27);
    let hover_text = hover["result"]["contents"].to_string();
    assert!(
        hover_text.contains("io.print(value) -> void"),
        "hover should include io.print signature: {}",
        hover
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 28,
            "method": "textDocument/signatureHelp",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 2, "character": 10 }
            }
        }),
    );
    let signature = read_response(&mut stdout, 28);
    assert_eq!(
        signature["result"]["signatures"][0]["label"],
        "io.print(value) -> void"
    );
    assert_eq!(signature["result"]["activeParameter"], 0);

    shutdown(child, &mut stdin, &mut stdout);
}

#[test]
fn lsp_context_completion_handles_imports_modules_and_invalid_source() {
    let dir = temp_project("context_completion");
    let main = dir.join("main.kiro");
    fs::write(&main, "import math\nmath.\n").expect("main file should be written");
    fs::write(
        dir.join("math.kiro"),
        "pure fn add(a: num, b: num) -> num { return a + b }\n",
    )
    .expect("math file should be written");

    let uri = file_uri(&main);
    let (child, mut stdin, mut stdout) = start_lsp(&dir);
    initialize(&mut stdin, &mut stdout, &dir);
    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "kiro",
                    "version": 1,
                    "text": "import math\nmath.\n"
                }
            }
        }),
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 30,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 1, "character": 5 }
            }
        }),
    );
    let completion = read_response(&mut stdout, 30);
    let labels = completion["result"]
        .as_array()
        .expect("module completion should be array")
        .iter()
        .filter_map(|item| item["label"].as_str())
        .collect::<Vec<_>>();
    assert!(
        labels.contains(&"add"),
        "module completion missing add: {:?}",
        labels
    );
    assert!(
        !labels.contains(&"fn"),
        "module completion should not include keywords: {:?}",
        labels
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": [{ "text": "import \nfn helper() -> num {\n    return 1\n}\nfn broken() {\n" }]
            }
        }),
    );
    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 31,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 0, "character": 7 }
            }
        }),
    );
    let completion = read_response(&mut stdout, 31);
    let labels = completion["result"]
        .as_array()
        .expect("import completion should be array")
        .iter()
        .filter_map(|item| item["label"].as_str())
        .collect::<Vec<_>>();
    assert!(
        labels.contains(&"math"),
        "sibling module missing: {:?}",
        labels
    );
    assert!(
        labels.contains(&"std_fs"),
        "std module missing: {:?}",
        labels
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 32,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 1, "character": 0 }
            }
        }),
    );
    let completion = read_response(&mut stdout, 32);
    let labels = completion["result"]
        .as_array()
        .expect("fallback completion should be array")
        .iter()
        .filter_map(|item| item["label"].as_str())
        .collect::<Vec<_>>();
    assert!(
        labels.contains(&"fn"),
        "invalid source should still offer keywords: {:?}",
        labels
    );
    assert!(
        labels.contains(&"helper"),
        "invalid source should still offer local parse-only symbols: {:?}",
        labels
    );

    shutdown(child, &mut stdin, &mut stdout);
}

#[test]
fn lsp_definition_does_not_return_fake_file_uri_for_embedded_std_modules() {
    let dir = temp_project("std_definition");
    let main = dir.join("main.kiro");
    fs::write(
        &main,
        r#"import std_fs

std_fs.exists("missing.txt")
"#,
    )
    .expect("main file should be written");

    let uri = file_uri(&main);
    let (child, mut stdin, mut stdout) = start_lsp(&dir);
    initialize(&mut stdin, &mut stdout, &dir);
    let source = fs::read_to_string(&main).expect("main source should read");
    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "kiro",
                    "version": 1,
                    "text": source
                }
            }
        }),
    );

    send_lsp(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 40,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 2, "character": 9 }
            }
        }),
    );
    let definition = read_response(&mut stdout, 40);
    assert!(
        definition["result"].is_null(),
        "embedded std definition should not point at a fake file: {}",
        definition
    );

    shutdown(child, &mut stdin, &mut stdout);
}
