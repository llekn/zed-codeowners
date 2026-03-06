# CODEOWNERS for Zed

A [Zed](https://zed.dev) extension that displays the CODEOWNERS of the currently focused file as an inline diagnostic hint.

Inspired by [vscode-codeowners](https://github.com/jasonnutter/vscode-codeowners).

## How it works

When you open a file in a project that has a CODEOWNERS file, a hint diagnostic appears at line 1 showing the file's owner(s):

```
ℹ Owner: @backend-team, @alice
```

The extension ships a lightweight LSP server (`codeowners-lsp`) that:

1. Finds the CODEOWNERS file (`.github/CODEOWNERS`, `CODEOWNERS`, or `docs/CODEOWNERS`)
2. Parses the rules using gitignore-style glob matching
3. Matches the current file path (last matching rule wins, per GitHub's precedence)
4. Publishes the result as a hint-level diagnostic

## Architecture

```
zed-codeowners/
├── extension.toml              # Zed extension manifest (registers LSP for 54 languages)
├── src/lib.rs                  # Zed extension (WASM) — locates and launches the LSP
├── codeowners-lsp/             # Standalone LSP server binary
│   └── src/
│       ├── main.rs             # LSP server (tower-lsp)
│       └── codeowners.rs       # CODEOWNERS parsing and file matching
└── languages/codeowners/       # Syntax highlighting for CODEOWNERS files
```

## Development

### Prerequisites

- Rust toolchain with `wasm32-wasip1` target: `rustup target add wasm32-wasip1`

### Build the LSP server

```sh
cd codeowners-lsp
cargo build --release
```

### Install the LSP binary

```sh
cargo install --path codeowners-lsp
```

### Build the Zed extension

```sh
cargo build --target wasm32-wasip1 --release
```

### Try it locally

1. Install the `codeowners-lsp` binary (see above)
2. Install as a [Zed dev extension](https://zed.dev/docs/extensions/developing-extensions)
3. Open a project with a CODEOWNERS file
4. Open any source file — you should see an owner hint at line 1

## License

MIT
