# rome-archivetool

`rome-archivetool` is a Rust CLI and library for inspecting, extracting, creating, and updating Bethesda BSA and BA2 archives.

The tool is intentionally designed as a reusable library first, with a thin CLI wrapper. It uses Ryan McKenzie's `ba2` crate for archive format support.

## Goals

- List archive contents in human-readable or JSON form.
- Extract single files or whole archives safely.
- Create TES3 BSA, TES4 BSA, and FO4/Starfield BA2 archives.
- Add or update archive entries by writing a new archive output.
- Expose an optional Lua embedding API behind the `lua` feature.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo clippy --all-targets --features lua -- -D warnings
cargo test --features lua
```

## License

GPL-3.0-or-later. See `LICENSE`.
