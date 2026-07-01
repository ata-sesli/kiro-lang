# Kiro Zed Extension

Local-only Zed language support for `.kiro` files.

## Local install

1. Open Zed.
2. Open the command palette.
3. Run `zed: install dev extension`.
4. Select `/Users/atasesli/Desktop/VsCode/kiro-lang/kiro-zed`.
5. Open any `.kiro` file.

Zed should select the `Kiro` language automatically for `.kiro` files.

## Included support

- File association for `.kiro`.
- `//` line comments.
- Four-space indentation.
- Bracket pairing for `{}`, `[]`, `()`, double quotes, and single quotes.
- Tree-sitter parsing for Kiro syntax used by this repository.
- Syntax highlighting, bracket matching, indentation queries, outline entries, text objects, and override captures.
- Kiro LSP integration through `kiro lsp` for save-time diagnostics, formatting, hover docs, basic completions, and document symbols.

The extension launches `kiro-lang lsp` from `PATH`, falling back to `kiro lsp` when available. Keep Tree-sitter responsible for syntax/highlighting and keep semantic language rules in the compiler-backed LSP.

## Grammar source

The local grammar lives in `tree-sitter-kiro`. `extension.toml` points Zed to that folder with a `file://` grammar repository URL.
That folder is initialized as a local Git repository on branch `main`, because Zed grammar entries require a Git revision.

After grammar edits, regenerate parser files:

```sh
cd /Users/atasesli/Desktop/VsCode/kiro-lang/kiro-zed/tree-sitter-kiro
tree-sitter generate
git add .
git commit -m "Update Kiro grammar"
```

Basic parser verification:

```sh
cd /Users/atasesli/Desktop/VsCode/kiro-lang
tree-sitter parse --grammar-path kiro-zed/tree-sitter-kiro --quiet --json-summary $(rg --files -g '*.kiro')
```
