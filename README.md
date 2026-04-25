# rome-archivetool

`rome-archivetool` is a Rust CLI and library for inspecting, extracting, creating, and updating Bethesda BSA and BA2 archives.

The tool is intentionally designed as a reusable library first, with a thin CLI wrapper. It uses Ryan McKenzie's `ba2` crate for archive format support.

## Goals

- List archive contents in human-readable or JSON form.
- Extract single files or whole archives safely.
- Create TES3 BSA, TES4 BSA, and FO4/Starfield BA2 archives.
- Add or update archive entries by writing a new archive output.
- Expose an optional Lua embedding API behind the `lua` feature.

## Usage

```bash
rome-archivetool info archive.bsa
rome-archivetool info --json archive.bsa
rome-archivetool list archive.bsa
rome-archivetool list --long archive.bsa
rome-archivetool list --json archive.bsa
rome-archivetool extract archive.bsa textures/example.dds --output out/
rome-archivetool extract archive.bsa textures/example.dds --stdout > example.dds
rome-archivetool extract-all archive.bsa --output out/
rome-archivetool create out.bsa input_dir/ --format tes3
rome-archivetool create out.bsa input_dir/ --format tes4 --tes4-version oblivion
rome-archivetool create out.ba2 input_dir/ --format fo4 --ba2-kind gnrl
rome-archivetool add base.bsa new_file.txt --output updated.bsa
rome-archivetool --generate-completion bash > rome-archivetool.bash
rome-archivetool --generate-manpage > rome-archivetool.1
```

## Library

The crate exposes `ArchiveTool` and option structs for reuse by other applications:

```rust,no_run
use rome_archivetool::{ArchiveTool, CreateOptions};

# fn main() -> rome_archivetool::Result<()> {
let entries = ArchiveTool::list("Morrowind.bsa")?;
let bytes = ArchiveTool::read_entry("Morrowind.bsa", "icons/gold.dds")?;
let count = ArchiveTool::create("out.bsa", "input", &CreateOptions::default())?;
# Ok(())
# }
```

## Lua

Enable the `lua` feature to embed the API in an existing Lua state:

```rust,no_run
use mlua::Lua;

# fn main() -> mlua::Result<()> {
let lua = Lua::new();
rome_archivetool::lua::register(&lua)?;
# Ok(())
# }
```

The registered `rome_archivetool` table exposes `guess_format`, `info`, `list`, `extract`, `extract_all`, `create`, and `add`.

## Safety

Extraction rejects absolute paths and `..` components before writing files. Existing targets fail by default; pass `--overwrite` or `--skip-existing` to choose another policy.

Archive creation and update write to a temporary file in the output directory, then rename it into place after a successful write. Failed writes should not clobber an existing output archive.

## Performance

Archives are opened once per high-level operation. `extract-all` streams entries to disk as it iterates the loaded archive instead of buffering the whole archive payload, reopening, reparsing, or doing list-plus-lookup scans for each file. `--skip-existing` checks the destination before decoding entry payloads. `add` also iterates the loaded archive directly and skips decoding existing entries that are replaced by new inputs.

Creation and update currently stage output archive entries in memory before writing because the `ba2` writer APIs build archive maps before serialization. This is acceptable for initial use, but very large archive creation or update can require substantial memory.

## Format Notes

- `add` writes a new archive and preserves the source archive's TES4/FO4 write options directly where the `ba2` crate exposes them, including BA2 version variants such as Starfield v3 and Fallout 4 next-gen v8.
- `create --format fo4 --ba2-kind gnrl` is the general-purpose BA2 mode and accepts any file names.
- `create --format fo4 --ba2-kind dx10` only accepts `.dds` entries. This extension check is case-insensitive; the underlying writer may still reject invalid DDS data.
- `create --format fo4 --ba2-kind gnmf` only accepts `.gnf` entries. This extension check is case-insensitive; the underlying writer may still reject invalid GNF data.
- FO4/BA2 archives are written with string tables enabled so entries can be listed and extracted by path later.
- TES4 BSA creation defaults to miscellaneous archive type flags.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo clippy --all-targets --features lua -- -D warnings
cargo test --features lua
cargo bench --bench archive_ops
```

Use `cargo bench --bench archive_ops` to profile generated synthetic archives for listing, single-entry lookup, whole-archive extraction, skip-existing extraction, creation, and update paths. On Linux, `/usr/bin/time -v cargo bench --bench archive_ops` is useful for checking peak resident memory while tuning large archive operations.

## License

GPL-3.0-or-later. See `LICENSE`.
