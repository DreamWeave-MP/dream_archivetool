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
# extract/extract-all default to the current directory when --output is omitted
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
        fsync: false,
    },
)?;
let all = ArchiveTool::extract_all("Morrowind.bsa", &ExtractAllOptions::default())?;
let created = ArchiveTool::create("out.bsa", "input", &CreateOptions::default())?;
let updated = ArchiveTool::add(
    "out.bsa",
    &AddOptions {
        inputs: vec!["new_file.txt".into()],
        output: "updated.bsa".into(),
        fsync: false,
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
- `list(path) -> { { path, path_bytes_hex, size, compressed_size }, ... }`
- `read_entry(path, entry) -> string`
- `extract(path, entry, opts?) -> { extracted, skipped }`
- `extract_all(path, opts?) -> { extracted, skipped }`
- `create(output, input, opts?) -> file_count`
- `add(path, opts) -> file_count`

Lua option tables:

- `extract`: `output`, `overwrite`, `preserve_paths`, `fsync`
- `extract_all`: `output`, `overwrite`, `fsync`
- `create`: `format`, `tes4_version`, `ba2_kind`, `ba2_version`, `fsync`
- `add`: `output`, `inputs`, `fsync`

## Safety

Extraction rejects absolute paths and `..` components before writing files. Existing targets fail by default; pass `--overwrite` or `--skip-existing` to choose another policy. `extract` and `extract-all` write under the current directory when `--output` is omitted.

Archive creation and update write to a temporary file in the output directory, then rename it into place after a successful write. Failed writes should not clobber an existing output archive.

## Performance

Archives are opened once per high-level operation. Single-file extraction and `extract-all` stream entry payloads into their output writer through `dream_archive` instead of first materializing whole files in `dream-archivetool`. `extract-all` checks the destination before decoding entry payloads, so `--skip-existing` avoids reading skipped files. `add` skips decoding existing entries that are replaced by new inputs. Directory inputs for `create` and `add` are stored relative to each directory root; the root directory name itself is not preserved.

Archive creation and update preflight archive paths and format policy before reading payload bytes, but currently stage output archive entries in memory before writing because the `dream_archive` writer APIs build archive maps before serialization. This is acceptable for initial use, but very large archive creation or update can require substantial memory until the backend grows deferred source/reader builder APIs.

## Format Notes

- `add` writes a new archive and preserves the source archive's TES4/FO4 write options directly where `dream_archive` exposes them, including BA2 version variants such as Starfield v3 and Fallout 4 next-gen v8. Archives with entries that do not have recoverable path names, including TES4 hash-only archives, are rejected rather than rewritten lossy.
- Format-specific `create` options are rejected with other formats: `--tes4-version` only applies to `--format tes4`, while `--ba2-kind` and `--ba2-version` only apply to `--format fo4`.
- `list --json` includes `path` as a lossy display string and `path_bytes_hex` as the normalized archive path bytes for scripts that must round-trip non-UTF-8 Unix names.
- `create --format fo4 --ba2-kind gnrl` is the general-purpose BA2 mode and accepts any file names.
- `create --format fo4 --ba2-kind dx10` only accepts `.dds` entries. This extension check is case-insensitive; the underlying writer may still reject invalid DDS data.
- `create --format fo4 --ba2-kind gnmf` is accepted by argument parsing but rejected before writing. GNMF writing requires console texture swizzle semantics that `dream_archive` intentionally does not implement yet.
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
