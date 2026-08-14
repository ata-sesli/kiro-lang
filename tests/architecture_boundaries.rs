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
