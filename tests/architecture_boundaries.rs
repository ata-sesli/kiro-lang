use std::fs;
use std::path::Path;

#[test]
fn backend_neutral_ir_layers_do_not_depend_on_parser_types() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    for layer in ["hir", "eir"] {
        let layer_root = source_root.join(layer);
        if layer_root.exists() {
            assert_no_grammar_dependency(&layer_root);
        }
    }
}

#[test]
fn source_tree_uses_domain_facades_with_nested_implementations() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    for module in [
        "compiler",
        "eir",
        "engine",
        "errors",
        "grammar",
        "hir",
        "interpreter",
    ] {
        assert!(
            source_root.join(format!("{module}.rs")).is_file(),
            "{module} should have a named subsystem facade"
        );
        assert!(
            !source_root.join(module).join("mod.rs").exists(),
            "{module} should use the modern foo.rs + foo/ layout"
        );
    }

    for nested in [
        "cli/app.rs",
        "cli/build.rs",
        "cli/project.rs",
        "cli/test_runner.rs",
        "compiler/rust_backend.rs",
        "host_generator/render.rs",
        "interpreter/eir_runtime/error.rs",
        "lsp/symbols.rs",
    ] {
        assert!(
            source_root.join(nested).is_file(),
            "expected subsystem implementation at {nested}"
        );
    }

    let main = fs::read_to_string(source_root.join("main.rs")).expect("main.rs should be readable");
    assert!(main.contains("kiro_lang::cli::app::run()"));
    assert!(
        main.lines().count() <= 5,
        "binary entry point should stay thin"
    );
}

fn assert_no_grammar_dependency(path: &Path) {
    for entry in fs::read_dir(path).expect("IR layer directory should be readable") {
        let entry = entry.expect("IR layer entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            assert_no_grammar_dependency(&path);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }

        let source = fs::read_to_string(&path).expect("IR source should be readable");
        assert!(
            !source.contains("crate::grammar") && !source.contains("super::grammar"),
            "{} must not depend on parser AST types",
            path.display()
        );
    }
}
