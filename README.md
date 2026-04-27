# dream-archivetool

`dream-archivetool` is a Rust CLI and library for inspecting, extracting, creating, and updating Bethesda BSA and BA2 archives.

The tool is intentionally designed as a reusable library first, with a thin CLI wrapper. It uses the `dream_archive` crate for archive format support.

## Goals

- List archive contents in human-readable or JSON form.
- Extract single files or whole archives safely.
- Create TES3 BSA, TES4 BSA, and FO4/Starfield BA2 archives.
- Add or update archive entries by writing a new archive output.
- Expose an optional Lua embedding API behind the `lua` feature.

## Usage

```bash
dream-archivetool info archive.bsa
dream-archivetool info --json archive.bsa
dream-archivetool list archive.bsa
dream-archivetool list --long archive.bsa
dream-archivetool list --json archive.bsa
dream-archivetool extract archive.bsa textures/example.dds --output out/
dream-archivetool extract archive.bsa textures/example.dds --stdout > example.dds
dream-archivetool extract-all archive.bsa --output out/
dream-archivetool create out.bsa input_dir/ --format tes3
dream-archivetool create out.bsa input_dir/ --format tes4 --tes4-version oblivion
dream-archivetool create out.ba2 input_dir/ --format fo4 --ba2-kind gnrl
dream-archivetool add base.bsa new_file.txt --output updated.bsa
dream-archivetool --generate-completion bash > dream-archivetool.bash
dream-archivetool --generate-manpage > dream-archivetool.1
```

## Library

The crate exposes `ArchiveTool` and option structs for reuse by other applications:

```rust,no_run
use dream_archivetool::{
    AddOptions, ArchiveTool, CreateOptions, ExtractAllOptions, ExtractOptions, OverwriteMode,
};

# fn main() -> dream_archivetool::Result<()> {
let entries = ArchiveTool::list("Morrowind.bsa")?;
let bytes = ArchiveTool::read_entry("Morrowind.bsa", "icons/gold.dds")?;
let extracted = ArchiveTool::extract(
    "Morrowind.bsa",
    "icons/gold.dds",
    &ExtractOptions {
        output: Some("out".into()),
        overwrite: OverwriteMode::Fail,
        preserve_paths: true,
    },
)?;
let all = ArchiveTool::extract_all("Morrowind.bsa", &ExtractAllOptions::default())?;
let created = ArchiveTool::create("out.bsa", "input", &CreateOptions::default())?;
let updated = ArchiveTool::add(
    "out.bsa",
    &AddOptions {
        inputs: vec!["new_file.txt".into()],
        output: "updated.bsa".into(),
    },
)?;
# Ok(())
# }
```

## Lua

Enable the `lua` feature to embed the API in an existing Lua state:

```rust,no_run
use mlua::Lua;

# fn main() -> mlua::Result<()> {
let lua = Lua::new();
dream_archivetool::lua::register(&lua)?;
# Ok(())
# }
```

The registered `dream_archivetool` table mirrors the public `ArchiveTool` API:

```lua
local tool = dream_archivetool

local format = tool.guess_format("Morrowind.bsa")
local info = tool.info("Morrowind.bsa")
local entries = tool.list("Morrowind.bsa")
local bytes = tool.read_entry("Morrowind.bsa", "icons/gold.dds")

local extracted = tool.extract("Morrowind.bsa", "icons/gold.dds", {
  output = "out",
  overwrite = "fail", -- fail | overwrite | skip
  preserve_paths = true,
})

local all = tool.extract_all("Morrowind.bsa", {
  output = "out",
  overwrite = "skip",
})

local created = tool.create("out.ba2", "input", {
  format = "fo4", -- tes3 | tes4 | fo4
  ba2_kind = "gnrl", -- gnrl | dx10 | gnmf
  ba2_version = "fallout4", -- fallout4 | starfield | fallout4-next-gen
})

local updated = tool.add("out.ba2", {
  output = "updated.ba2",
  inputs = { "new_file.txt", "new_dir" },
})
```

Lua functions and return values:

- `guess_format(path) -> "tes3" | "tes4" | "fo4"`
- `info(path) -> { path, format, file_count }`
- `list(path) -> { { path, size, compressed_size }, ... }`
- `read_entry(path, entry) -> string`
- `extract(path, entry, opts?) -> { extracted, skipped }`
- `extract_all(path, opts?) -> { extracted, skipped }`
- `create(output, input, opts?) -> file_count`
- `add(path, opts) -> file_count`

Lua option tables:

- `extract`: `output`, `overwrite`, `preserve_paths`
- `extract_all`: `output`, `overwrite`
- `create`: `format`, `tes4_version`, `ba2_kind`, `ba2_version`
- `add`: `output`, `inputs`

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
