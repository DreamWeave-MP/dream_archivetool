# dream-archivetool

`dream-archivetool` is a Rust CLI and library for inspecting, extracting, creating, and updating Bethesda BSA and BA2 archives.

The tool is intentionally designed as a reusable library first, with a thin CLI wrapper. It uses the `dream_archive` crate for archive format support.

## Goals

- List archive contents in human-readable or JSON form.
- Extract single files or whole archives safely.
- Create TES3 BSA, TES4 BSA, and BA2/Starfield BA2 archives.
- Add or update archive entries by writing a new archive output.
- Keep CLI parsing dependencies behind the default `cli` feature so GUI/library consumers can opt out.
- Expose an optional Lua embedding API behind the `lua` feature.

## Usage

```bash
dream-archivetool info archive.bsa
dream-archivetool info --json archive.bsa
dream-archivetool list archive.bsa
dream-archivetool list --long archive.bsa
dream-archivetool list --json archive.bsa
dream-archivetool verify archive.bsa --read-payloads --json
dream-archivetool diff old.bsa new.bsa --hash --json
dream-archivetool extract archive.bsa textures/example.dds --output out/
dream-archivetool extract archive.bsa textures/example.dds --stdout > example.dds
dream-archivetool extract archive.bsa --entry-hex 74657874757265732f6578616d706c652e646473 --stdout > example.dds
dream-archivetool extract-all archive.bsa --output out/ --dry-run --json
dream-archivetool extract-all archive.bsa --output out/
# extract/extract-all default to the current directory when --output is omitted
dream-archivetool create out.bsa input_dir/ --format tes3
dream-archivetool create out.bsa input_dir/ --format tes4 --tes4-version oblivion
dream-archivetool create out.ba2 input_dir/ --format ba2 --ba2-kind gnrl
dream-archivetool add base.bsa new_file.txt --output updated.bsa
dream-archivetool add base.bsa new_dir/ --output updated.bsa --dry-run --json
dream-archivetool --generate-completion bash > dream-archivetool.bash
dream-archivetool --generate-manpage > dream-archivetool.1
```

## CLI Contracts

Running `dream-archivetool` without a subcommand intentionally prints top-level help and exits successfully. Argument parsing errors still use clap's nonzero misuse exit code, and runtime archive/file failures return a runtime error.

Human output is for people. JSON output is the scripting contract and is written to stdout without progress text; diagnostics go to stderr in the binary. `extract --stdout` writes only payload bytes to stdout and conflicts with JSON and disk-write options.

### Archive Path Contract

Archive paths are normalized as virtual paths using `/` separators. Matching treats `\` as `/` and follows the case-normalized lookup behavior exposed by the archive backend. Extraction rejects absolute paths and `..` components before writing files.

`list --json` reports both a display path and exact normalized path bytes:

```json
[
  {
    "path": "textures/example.dds",
    "path_bytes_hex": "74657874757265732f6578616d706c652e646473",
    "size": 123,
    "compressed_size": null
  }
]
```

`path` is a lossy display string for humans. `path_bytes_hex` is the stable round-trip value for scripts, including non-UTF-8 Unix archive names. Feed it back with `extract --entry-hex HEX`; otherwise pass a positional entry path. Positional non-UTF-8 entry bytes are accepted on Unix through raw argv bytes.

Directory inputs for `create` and `add` are stored relative to each directory root:

```text
input_dir/textures/a.dds -> textures/a.dds
```

The root directory name itself is not stored unless it is part of the path below the input root.

### JSON Shapes

`info --json`:

```json
{
  "path": "archive.bsa",
  "format": "tes3",
  "file_count": 2,
  "named_entry_count": 2,
  "has_unnameable_entries": false,
  "rewritable": true
}
```

`extract --json` and `extract-all --json`:

```json
{ "extracted": 1, "skipped": 0 }
```

`verify --json` reports archive health, duplicate/unsafe path issues, rewrite blockers,
and optional payload-read counts when `--read-payloads` is used.

`create --json` and `add --json`:

```json
{ "files": 2 }
```

`create --dry-run --json`, `add --dry-run --json`, and
`extract-all --dry-run --json` expose the same normalized paths and policy checks
the mutating commands use, but stop before writing output.

## Library

The crate exposes `ArchiveTool` and option structs for reuse by other applications:

GUI or embedding projects that do not need the command-line interface should disable default features to avoid pulling in `clap`, completion generation, and manpage generation dependencies:

```toml
[dependencies]
dream-archivetool = { version = "0.1", default-features = false }
```

The `cli` feature is enabled by default for building the `dream-archivetool` binary. Add `features = ["lua"]` if the embedding API is needed.

```rust,no_run
use dream_archivetool::{
    AddOptions, ArchiveTool, CreateOptions, ExtractAllOptions, ExtractOptions, OverwriteMode,
};

# fn main() -> dream_archivetool::Result<()> {
let archive = ArchiveTool::open("Morrowind.bsa")?;
let entries = archive.list()?;
let bytes = archive.read_entry("icons/gold.dds")?;
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

The registered `dream_archivetool` table is a small Lua facade over common `ArchiveTool` operations:

```lua
local tool = dream_archivetool

local format = tool.guess_format("Morrowind.bsa")
local info = tool.info("Morrowind.bsa")
local entries = tool.list("Morrowind.bsa")
local bytes = tool.read_entry("Morrowind.bsa", "icons/gold.dds")
local exact_bytes = tool.read_entry_hex("Morrowind.bsa", entries[1].path_bytes_hex)

local extracted = tool.extract("Morrowind.bsa", "icons/gold.dds", {
  output = "out",
  overwrite = "fail", -- fail | overwrite | skip
  preserve_paths = true,
})

local exact_extracted = tool.extract_hex("Morrowind.bsa", entries[1].path_bytes_hex, {
  output = "out",
  overwrite = "fail",
  preserve_paths = true,
})

local all = tool.extract_all("Morrowind.bsa", {
  output = "out",
  overwrite = "skip",
})

local created = tool.create("out.ba2", "input", {
  format = "ba2", -- tes3 | tes4 | ba2
  ba2_kind = "gnrl", -- gnrl | dx10 | gnmf
  ba2_version = "fallout4", -- fallout4 | starfield | fallout4-next-gen
})

local updated = tool.add("out.ba2", {
  output = "updated.ba2",
  inputs = { "new_file.txt", "new_dir" },
})
```

Lua functions and return values:

- `guess_format(path) -> "tes3" | "tes4" | "ba2"`
- `info(path) -> { path, format, file_count }`
- `list(path) -> { { path, path_bytes_hex, size, compressed_size }, ... }`
- `read_entry(path, entry) -> string`
- `read_entry_hex(path, path_bytes_hex) -> string`
- `extract(path, entry, opts?) -> { extracted, skipped }`
- `extract_hex(path, path_bytes_hex, opts?) -> { extracted, skipped }`
- `extract_all(path, opts?) -> { extracted, skipped }`
- `create(output, input, opts?) -> file_count`
- `add(path, opts) -> file_count`

Lua option tables:

- `extract`: `output`, `overwrite`, `preserve_paths`, `fsync`
- `extract_all`: `output`, `overwrite`, `fsync`
- `create`: `format`, `tes4_version`, `ba2_kind`, `ba2_version`, `fsync`
- `add`: `output`, `inputs`, `fsync`

## Safety

Extraction rejects absolute paths, `..` components, NUL bytes, and colon-containing components before writing files. Existing targets fail by default; pass `--overwrite` or `--skip-existing` to choose another policy. `extract` and `extract-all` write under the current directory when `--output` is omitted. These checks validate archive path syntax; they are not an `openat`-style filesystem jail, so extract into an output tree whose pre-existing directories and symlinks you trust.

Archive creation and update write to a temporary file in the output directory, then rename it into place after a successful write. Failed writes should not clobber an existing output archive.

## Performance

The stateless `ArchiveTool` facade opens archives once per high-level operation; use `ArchiveTool::open` / `OpenArchive` for repeated list/read/extract calls against the same archive. Single-file extraction and `extract-all` stream entry payloads into their output writer through `dream_archive` instead of first materializing whole files in `dream-archivetool`. `extract-all` checks the destination before decoding entry payloads, so `--skip-existing` avoids reading skipped files. Directory inputs for `create` and `add` are stored relative to each directory root; the root directory name itself is not preserved.

Archive creation and update preflight archive paths and format policy before adding deferred payload sources to the backend builders. TES3, TES4, and BA2 GNRL creation pass filesystem paths to `dream_archive` rather than preloading payloads in `dream-archivetool`. `add` preserves unchanged TES3, TES4, and BA2 GNRL entries from the source archive through deferred archive-entry sources instead of decoding them into `dream-archivetool` memory. BA2 DX10 still has to parse DDS texture data through the backend texture builder, and preserved entries are buffered during rewrite because the texture builder currently has no archive-entry preservation API. Annoying, but at least now the code says what it is doing.

## Format Notes

- `add` writes a new archive and preserves the source archive's TES4/BA2 write options directly where `dream_archive` exposes them, including BA2 version variants such as Starfield v2 and Fallout 4 next-gen v8. Archives with entries that do not have recoverable path names, including TES4 hash-only archives, are rejected rather than rewritten lossy.
- Format-specific `create` options are rejected with other formats: `--tes4-version` only applies to `--format tes4`, while `--ba2-kind` and `--ba2-version` only apply to `--format ba2`.
- `list --json` includes `path` as a lossy display string and `path_bytes_hex` as the normalized archive path bytes for scripts that must round-trip non-UTF-8 Unix names. Use `extract --entry-hex HEX` to feed those bytes back into the CLI.
- `create --format ba2 --ba2-kind gnrl` is the general-purpose BA2 mode and accepts any file names.
- `create --format ba2 --ba2-kind dx10` only accepts `.dds` entries. This extension check is case-insensitive; the underlying writer may still reject invalid DDS data.
- `create --format ba2 --ba2-kind gnmf` is accepted by argument parsing but rejected before writing. GNMF writing requires console texture swizzle semantics that `dream_archive` intentionally does not implement yet.
- BA2 archives are written with string tables enabled so entries can be listed and extracted by path later.
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
