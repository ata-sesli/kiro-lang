use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/grammar/mod.rs");
    rust_sitter_tool::build_parsers(&PathBuf::from("src/grammar/mod.rs"));
}
