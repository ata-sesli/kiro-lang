<p align="center">
  <img src="kiro-logo.png" width="300" alt="Kiro Logo">
</p>

# Kiro Language Support for VS Code

![Maintenance](https://img.shields.io/badge/maintenance-community--maintained-yellow)

> Maintenance status: community-maintained.
>
> I no longer use VS Code as my daily editor, so I do not plan to actively develop this extension myself. The extension remains available, and pull requests are welcome from anyone who wants to improve or maintain VS Code support for Kiro. My own editor tooling focus is now the Zed extension.

Visual Studio Code extension for **Kiro**, an experimental programming language focused on clear syntax, explicit control flow, and practical host integration.

## Features

### Syntax Highlighting
The extension ships with a TextMate grammar for `.kiro` files and highlights:

- Core declarations: `fn`, `var`, `struct`, `import`, `error`, `rust`, `pure`
- Control flow: `on`, `off`, `loop`, `in`, `per`, `break`, `continue`, `return`, `check`, `run`, `rest`
- Type keywords: `num`, `str`, `bool`, `void`, `adr`, `pipe`, `list`, `map`
- Operators and commands: `ref`, `deref`, `move`, `give`, `take`, `close`, `at`, `push`, `len`
- Strings, comments, punctuation, and grouping tokens

### Hover Documentation
Built-in hover docs are provided for:

- Language keywords and core constructs
- Standard modules such as `std_fs`, `std_env`, `std_net`, `std_time`, and `std_io`
- Qualified module calls like `std_fs.read` and `std_io.input`

### Language Configuration
- Bracket pairing for `{}`, `()`, and `[]`
- `//` line comment support
- Proper language registration for `*.kiro`

## Installation

### Local Development (from this repository)

1. Open `kiro-vscode/kiro` in VS Code.
2. Run `npm install` if your environment requires dependencies for extension packaging workflows.
3. Press `F5` to launch an **Extension Development Host**.
4. Open a `.kiro` file in the new window.

### VSIX Packaging (optional)

If you want to distribute/test as a package:

1. Install `vsce`.
2. From `kiro-vscode/kiro`, run `vsce package`.
3. Install the generated `.vsix` from VS Code extensions UI.

## Usage

Create any file with the `.kiro` extension:

```kiro
import io

fn main() -> void {
    io.print("Hello, Kiro!")

    var x = 10
    on (x > 5) {
        io.print("Greater than 5")
    } off {
        io.print("5 or less")
    }
}
```

Hover over keywords (for example `loop`, `ref`, `pipe`) or standard module members (for example `std_io.input`) to view inline docs.

## Project Layout

- `package.json`: extension manifest and contributions
- `extension.js`: hover provider registration and activation logic
- `docs/hoverDocs.js`: keyword/module hover documentation source
- `syntaxes/kiro.tmLanguage.json`: TextMate grammar
- `language-configuration.json`: bracket and comment rules

## Current Scope

This extension currently focuses on **language-authoring essentials**:

- Syntax highlighting
- Hover help
- Baseline language configuration

It does **not** currently include full semantic tooling such as:

- Diagnostics from compiler output
- Go-to-definition
- Rename symbol
- Semantic tokens
- Formatting

Those are natural next steps when/if an LSP server is introduced.

This extension is intentionally in maintenance mode. New Kiro language features may not appear here immediately unless contributors update the TextMate grammar and hover docs.

## Contributing

Contributions are welcome. Good contributions include:

- Improving grammar precision in `syntaxes/kiro.tmLanguage.json`
- Expanding hover docs in `docs/hoverDocs.js`
- Tightening language config behavior
- Keeping docs aligned with language evolution

If you use VS Code and notice tooling drift after a Kiro language change, PRs are welcome. Good PRs should include updates to the TextMate grammar, hover docs, and README examples when relevant.

## Kiro Language

Main language repository:

- https://github.com/dark-mesh/kiro-lang

Tutorial docs in this repo:

- `learn-kiro/`

## License

MIT
