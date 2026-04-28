# dream-archivetool

`dream-archivetool` is a Rust CLI and library for inspecting, extracting, creating, and updating Bethesda BSA and BA2 archives.

The tool is intentionally designed as a reusable library first, with a thin CLI wrapper. It uses the `dream_archive` crate for archive format support. See [`docs/architecture.md`](docs/architecture.md) for subsystem ownership, compatibility contracts, and Lua handle semantics.

## Goals

- List archive contents in human-readable or JSON form.
- Extract single files or whole archives safely.
- Create TES3 BSA, TES4 BSA, and BA2/Starfield BA2 archives.
- Add or update archive entries by rewriting the archive, optionally to a separate output.
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
dream-archivetool extract-all archive.bsa --output out/ --dry-run
dream-archivetool extract-all archive.bsa --output out/
# extract/extract-all default to the current directory when --output is omitted
dream-archivetool create out.bsa input_dir/ --format tes3
dream-archivetool create out.bsa input_dir/ --format tes4 --tes4-version oblivion
dream-archivetool create out.ba2 input_dir/ --format ba2 --ba2-kind gnrl
dream-archivetool create out.bsa input_dir/ --format tes3 --follow-symlinks
dream-archivetool add base.bsa new_file.txt
dream-archivetool add base.bsa new_file.txt --output updated.bsa
dream-archivetool add base.bsa new_dir/ --output updated.bsa --dry-run
dream-archivetool --generate-completion bash > dream-archivetool.bash
dream-archivetool --generate-manpage > dream-archivetool.1
```

## CLI Contracts

Running `dream-archivetool` without a subcommand intentionally prints top-level help and exits successfully. Argument parsing errors still use clap's nonzero misuse exit code, and runtime archive/file failures return a runtime error.

Human output is for people. JSON output is the scripting contract and is written to stdout without progress text; diagnostics go to stderr in the binary. `extract --stdout` writes only payload bytes to stdout and conflicts with JSON and disk-write options. JSON compatibility follows the crate's semver contract; additive fields may appear in minor releases, while field removals or renames require a breaking release.

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

`path` is a lossy display string for humans. `path_bytes_hex` is the hex-encoded normalized archive-path lookup key emitted by this tool. Feed it back with `extract --entry-hex HEX`; otherwise pass a positional entry path. It is not raw archive-name identity: distinct raw names may normalize to the same lookup key. Use `verify` to detect duplicate normalized paths, and use lower-level raw/index archive APIs when raw identity matters. Positional non-UTF-8 entry bytes are accepted on Unix through raw argv bytes.

Directory inputs for `create` and `add` are stored relative to each directory root:

```text
input_dir/textures/a.dds -> textures/a.dds
```

The root directory name itself is not stored unless it is part of the path below the input root.
Symbolic links encountered during input collection are rejected by default; pass
`--follow-symlinks` only when the input tree is trusted, stable during the write, and packaging the
symlink target bytes is intentional.

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
and optional payload-read counts when `--read-payloads` is used. Payload reads are skipped with a
warning when duplicate normalized paths prevent per-entry coverage.

`diff` without `--hash` is a metadata-only comparison and does not prove payload equality, especially for archive formats where the backend cannot expose complete size metadata. `diff --hash` streams payload bytes through a fast non-cryptographic FNV-1a fingerprint and reports it as `payload_fingerprint`; it is for change detection, not integrity or adversarial collision checks.

`create --json` and `add --json`:

```json
{ "files": 2 }
```

`create --dry-run`, `add --dry-run`, and
`extract-all --dry-run` print JSON plans exposing the same normalized paths and policy checks
the mutating commands use, but stop before writing output. The `--json` flag is accepted with dry-run commands for consistency, but dry-run output is already JSON. Add plans use stable report order grouped
by action; they are not a physical archive-order manifest.

## Library

The crate exposes `ArchiveTool` and option structs for reuse by other applications. Public report/plan DTOs and evolvable enums are marked non-exhaustive so new fields or variants can be added without pretending today's archive formats are the end of history. Option structs remain literal-friendly; prefer `..Default::default()` in application code so future major-version additions are easier to adopt.

GUI or embedding projects that do not need the command-line interface should disable default features to avoid pulling in `clap`, completion generation, manpage generation, and `serde_json` dependencies:

```toml
[dependencies]
dream-archivetool = { version = "0.1", default-features = false }
```

The `cli` feature is enabled by default for building the `dream-archivetool` binary. The binary target requires `cli`, so `cargo build --no-default-features` builds the library without producing a nonfunctional CLI stub. Add `features = ["lua"]` if the embedding API is needed. The `lua` feature enables this crate's Lua module and compatible Lua support in the re-exported `dream_archive` and `dream_path` APIs, but it does not choose a Lua runtime. Embedding applications should select the `mlua` runtime centrally. The `standalone-lua` feature enables vendored LuaJIT 5.2 for this crate's tests and docs, not for normal downstream use. The intended Lua stack is `dream_path` for virtual path helpers, `dream_archive` for archive mechanics, and `dream_archivetool` for filesystem/rewrite policy; `dream_archive` is re-exported as `dream_archivetool::dream_archive` so downstream users get the same crate and feature set this policy layer was compiled against.

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
        output: Some("updated.bsa".into()),
        fsync: false,
        follow_symlinks: false,
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
lua.globals().set(
    "dream_path",
    dream_archivetool::dream_archive::dream_path::lua::create_module(&lua)?,
)?;
lua.globals().set(
    "dream_archive",
    dream_archivetool::lua::create_dream_archive_module(&lua)?,
)?;
dream_archivetool::lua::register(&lua)?;
# Ok(())
# }
```

The registered `dream_archivetool` table exposes tool-policy operations that `dream_archive` does not: safe filesystem extraction, rewrite/create planning, verification, diff reports, temp-output mutation, symlink policy, and durability options. Archive-format primitives such as opening archives, listing entries, reading payload bytes, hash helpers, and builders belong to `dream_archive`'s Lua API. Use `dream_archivetool::lua::create_dream_archive_module(&lua)` instead of the plain `dream_archive::lua::create_module(&lua)` when you want those policy operations as methods on `dream_archive.open_*` archive userdata. Method registration must happen before creating archive userdata in that Lua state; `mlua` caches metatables by Rust type, because of course it does.

Lua string boundaries are deliberately split. Filesystem paths (`archive`, `output`, `input`, and source paths in `inputs`) are UTF-8 host paths. Archive entry paths are byte strings: `extract(path, entry, opts)` and `extract_many(path, entries, opts)` accept the raw Lua string bytes returned by `dream_archive` entry listings, while `extract_by_path_hex` / `extract_hex`, `extract_many_by_path_hex`, and `plan_extract_by_path_hex` accept the serialized `path_bytes_hex` lookup key used by archivetool reports. This bridge only applies to entries with a non-`nil` `entry.path`; hash-only or unnameable entries belong to lower-level `dream_archive` APIs and are not safe path-policy extraction targets. Display `path` fields are for people; `path_bytes_hex` is the stable normalized lookup key, not a promise that two raw archive names cannot collide after normalization. If duplicate normalized paths matter, use `verify` to detect them and drop to `dream_archive` raw/index APIs for archaeology. In `dream_archivetool` reports/plans, wide archive sizes (`size`, `compressed_size`) are decimal strings, not Lua numbers, because LuaJIT numbers are not a u64 transport. Counts such as `files`, `added`, and `extracted` remain Lua numbers for practical archive counts, not 64-bit identity values.

The `lua` feature enables compatible Lua support in `dream_archive`, including its re-exported `dream_path` helpers. Embedding applications should register `dream_path`, `dream_archive`, and `dream_archivetool` into the same Lua state when they need the whole stack. `dream_archivetool` does not install those dependency globals behind your back; hidden globals are how APIs become haunted furniture.

There are two Lua calling styles:

- `dream_archivetool.extract(path, ...)` is path-based. It opens the archive path for that operation.
- `archive:extract(...)` is handle-based. It runs policy against the already-opened `dream_archive` userdata.

That distinction matters. If you list entries from `dream_archive.open_path(path)` and then call a top-level `dream_archivetool` function with the same path, the file can change between those two operations. Use userdata methods when the operation should stay attached to the archive handle you already opened.

```lua
local tool = dream_archivetool

local info = tool.info("Morrowind.bsa")
local verify = tool.verify("Morrowind.bsa", { read_payloads = true })
local diff = tool.diff("old.bsa", "new.bsa", { fingerprint_payloads = true })

-- Byte-string bridge from dream_archive entry listings.
-- This top-level call reopens "Morrowind.bsa"; use archive:extract(...) below
-- when you want to operate on the already-opened handle.
-- Works for entries with non-nil entry.path.
local archive = dream_archive.open_path("Morrowind.bsa")
local entry = archive:entries()[1]
tool.extract("Morrowind.bsa", entry.path, { output = "out" })

-- If dream_archive was created through create_dream_archive_module, the same
-- policy can run directly on the already-opened archive userdata.
local verify_from_userdata = archive:verify({ read_payloads = true })
local selected_from_userdata = archive:extract_many({ entry.path }, { output = "selected-out" })
local userdata_plan = archive:plan_extract_by_path_hex({
  "69636f6e732f676f6c642e646473",
}, { output = "selected-out" })

-- Dry run: no files written.
local selected_plan = tool.plan_extract("Morrowind.bsa", { entry.path }, {
  output = "selected-out",
})

-- Stable report/plan bridge: use hex batch APIs when selecting from path_bytes_hex fields.
local selected_hex_plan = tool.plan_extract_by_path_hex("Morrowind.bsa", {
  "69636f6e732f676f6c642e646473",
}, { output = "selected-out" })

-- Mutating: writes the selected entries now, opening the archive once for the batch.
local selected = tool.extract_many("Morrowind.bsa", { entry.path }, {
  output = "selected-out",
})
local selected_by_hex = tool.extract_many_by_path_hex("Morrowind.bsa", {
  "69636f6e732f676f6c642e646473",
}, { output = "selected-out" })

-- Mutating: writes one file now.
local extracted = tool.extract("Morrowind.bsa", "icons/gold.dds", {
  output = "out",
  overwrite = "fail", -- fail | overwrite | skip
  preserve_paths = true,
})

local exact_extracted = tool.extract_by_path_hex("Morrowind.bsa", "69636f6e732f676f6c642e646473", {
  output = "out",
  overwrite = "fail",
  preserve_paths = true,
})

-- Dry run: no files written.
local extract_plan = tool.plan_extract_all("Morrowind.bsa", {
  output = "out",
  overwrite = "skip",
})
for _, entry in ipairs(extract_plan.entries) do
  if entry.action == "overwrite" then
    error("refusing to overwrite " .. entry.path)
  end
end
-- Mutating: writes files now.
local all = tool.extract_all("Morrowind.bsa", {
  output = "out",
  overwrite = "skip",
})

local create_plan = tool.plan_create("out.ba2", "input", {
  format = "ba2",
  ba2_kind = "gnrl",
})
-- Mutating: writes out.ba2 now. Review create_plan.entries first.
local created = tool.create("out.ba2", "input", {
  format = "ba2", -- bsa-tes3 | bsa-tes4 | ba2; tes3/tes4 aliases accepted
  ba2_kind = "gnrl", -- gnrl | dx10 | gnmf
  ba2_version = "fallout4", -- fallout4 | starfield | fallout4-next-gen
})

local add_plan = tool.plan_add("out.ba2", {
  inputs = { "new_file.txt", "new_dir" },
})
-- Mutating: rewrites out.ba2 now. Review add_plan.entries first.
local updated = tool.add("out.ba2", {
  inputs = { "new_file.txt", "new_dir" },
})
print(created.files, updated.files)
```

Lua functions and return values:

- `info(path) -> { path, format, file_count, named_entry_count, has_unnameable_entries, rewritable, rewrite_blocker, tes4?, ba2? }`
- `verify(path, opts?) -> { path, format, file_count, named_entry_count, unnameable_entries, rewritable, rewrite_blocker, duplicate_normalized_paths, unsafe_paths, payloads_read, warnings }`
- `diff(old, new, opts?) -> { old, new, comparison, fingerprint_payloads, added, removed, changed, unchanged }`
- `extract(path, entry_bytes, opts?) -> { extracted, skipped }`
- `extract_many(path, entry_bytes_array, opts?) -> { extracted, skipped }`
- `plan_extract(path, entry_bytes_array, opts?) -> { operation, archive, output, entries }`
- `extract_by_path_hex(path, path_bytes_hex, opts?) -> { extracted, skipped }`
- `extract_hex(path, path_bytes_hex, opts?) -> { extracted, skipped }` compatibility alias
- `extract_many_by_path_hex(path, path_bytes_hex_array, opts?) -> { extracted, skipped }`
- `plan_extract_by_path_hex(path, path_bytes_hex_array, opts?) -> { operation, archive, output, entries }`
- `extract_all(path, opts?) -> { extracted, skipped }`
- `plan_extract_all(path, opts?) -> { operation, archive, output, entries }`
- `create(output, input, opts?) -> { files }`
- `plan_create(output, input, opts?) -> { operation, format, output, files, entries }`
- `add(path, opts) -> { files }`
- `plan_add(path, opts) -> { operation, archive, output, format, files, added, replaced, preserved, entries }`

Lua archive userdata methods added by `create_dream_archive_module` / `register_dream_archive_methods`:

- `archive:tool_info() -> { path, format, file_count, named_entry_count, has_unnameable_entries, rewritable, rewrite_blocker, tes4?, ba2? }`
- `archive:verify(opts?) -> verify report`
- `archive:diff(other_archive, opts?) -> diff report`
- `archive:extract(entry_bytes, opts?) -> { extracted, skipped }`
- `archive:extract_many(entry_bytes_array, opts?) -> { extracted, skipped }`
- `archive:plan_extract(entry_bytes_array, opts?) -> extract plan`
- `archive:extract_by_path_hex(path_bytes_hex, opts?) -> { extracted, skipped }`
- `archive:extract_many_by_path_hex(path_bytes_hex_array, opts?) -> { extracted, skipped }`
- `archive:plan_extract_by_path_hex(path_bytes_hex_array, opts?) -> extract plan`
- `archive:extract_all(opts?) -> { extracted, skipped }`
- `archive:plan_extract_all(opts?) -> extract-all plan`

`tool_info` is deliberately named to avoid colliding with lower-level `dream_archive` userdata methods such as format/list/read operations. It returns archivetool policy metadata, including rewrite eligibility, not just archive-format metadata.

There are intentionally no `archive:create`, `archive:plan_create`, `archive:add`, or `archive:plan_add` methods. Creation and rewrite policy requires host filesystem paths, output selection, symlink policy, and temp-file replacement semantics, so those remain top-level `dream_archivetool` APIs.

Report and plan `format` values are aligned with `dream_archive`: `bsa-tes3`, `bsa-tes4`, or `ba2`. `create` / `plan_create` also accept the older `tes3` / `tes4` aliases. Entry tables use display `path` for humans and `path_bytes_hex` for normalized lookup. Never feed display `path` back as identity when non-UTF-8 archive names matter; use the `*_by_path_hex` functions or raw byte strings from `dream_archive`. Diff/archive-plan `size` and `compressed_size` values are decimal strings or `nil`. Unknown option keys are rejected so typos do not silently mutate the wrong thing. `add.output` is optional; omit it to replace the source archive after a successful full rewrite, or set it to write a separate archive. `add.inputs` is required and must be a dense Lua array sequence such as `{ "file", "dir" }`; dictionary keys and holes are errors.

Nested entry table shapes:

- verify path issue: `{ path, path_bytes_hex, raw_path_bytes_hex, colliding_raw_path_bytes_hex? }`
- info TES4 table: `{ version, archive_types, archive_types_bits, archive_flags, archive_flags_bits, unsupported_archive_flags_bits, name_mode }`
- info BA2 table: `{ version, payload_format, compression_format, strings }`
- diff entry: `{ path, path_bytes_hex, size?, compressed_size?, payload_fingerprint? }`
- diff change: `{ path, path_bytes_hex, old, new }`, where `old` / `new` are diff entry states
- extract plan entry: `{ action, path, path_bytes_hex, target }`, with `action = "extract" | "skip" | "overwrite"`
- create/add plan entry: `{ action, source?, path, path_bytes_hex, size? }`, with `action = "add" | "replace" | "preserve"`

Lua option tables:

- `extract`: `output`, `overwrite`, `preserve_paths`, `fsync`; `overwrite = "fail" | "overwrite" | "skip"`
- `extract_many`: same options as `extract`; `entries` must be a dense array of archive path byte strings
- `extract_many_by_path_hex`: same options as `extract`; `entries` must be a dense array of `path_bytes_hex` strings
- `extract_all`: `output`, `overwrite`, `fsync`; `overwrite = "fail" | "overwrite" | "skip"`
- `verify`: `read_payloads`
- `diff`: `fingerprint_payloads`
- `create`: `format`, `tes4_version`, `ba2_kind`, `ba2_version`, `fsync`, `follow_symlinks`; `format = "bsa-tes3" | "bsa-tes4" | "ba2"` (`"tes3"` / `"tes4"` aliases accepted); `tes4_version = "oblivion" | "fallout3" | "fallout-3" | "skyrim" | "skyrim-se" | "sse"`; `ba2_kind = "gnrl" | "dx10" | "gnmf"`; `ba2_version = "fallout4" | "fallout-4" | "starfield" | "fallout4-next-gen" | "fallout-4-next-gen"`
- `add`: `output`, `inputs`, `fsync`, `follow_symlinks`

`plan_extract`, `plan_extract_by_path_hex`, `plan_extract_all`, `plan_create`, and `plan_add` accept the same options as `extract_many`, `extract_many_by_path_hex`, `extract_all`, `create`, and `add` respectively. Plan results are advisory snapshots: they do not reserve archive contents, input directories, output paths, or symlink state. Mutating calls repeat policy checks and can still fail if the world changed. Yes, the filesystem is still a shared mutable global; c a p i t u l a t e.

Defaults: `format = "bsa-tes3"`, `tes4_version = "oblivion"`, `ba2_kind = "gnrl"`, `ba2_version = "fallout4"`, `overwrite = "fail"`, `preserve_paths = true`, `fsync = false`, `follow_symlinks = false`, omitted extraction `output` writes under the current directory, and omitted add `output` rewrites the source archive.

Path-based `dream_archivetool` Lua calls reopen the archive path passed to them. If you list with `dream_archive.open_path(path):entries()` and then call `dream_archivetool.extract_many(path, ...)`, the extraction is not bound to the already-opened `dream_archive` userdata; replacing the file between those two calls means you selected from one archive state and extracted from another. Userdata methods avoid that extra policy-layer reopen and operate on the supplied `dream_archive` handle. For byte-backed `dream_archive.open_bytes(...)` userdata this is a true immutable snapshot. For path-backed `dream_archive.open_path(...)` userdata, payload reads still follow `dream_archive`'s own source semantics; do not treat it as a magic file-handle pin unless the lower layer promises that. That is not a cache. That is a contract boundary.

## Safety

Extraction rejects absolute paths, `..` components, NUL bytes, and colon-containing components before writing files. Existing targets fail by default; pass `--overwrite` or `--skip-existing` to choose another policy. `extract` and `extract-all` write under the current directory when `--output` is omitted. These checks validate archive path syntax; they are not an `openat`-style filesystem jail, so extract into an output tree whose pre-existing directories and symlinks you trust.

Archive creation and update write to a temporary file in the output directory, then rename it into place after a successful write. For `add`, omitting `--output` replaces the source archive only after the full rewritten archive has been produced; this is not patch-in-place mutation. Failed writes should not clobber an existing output archive. Input symlinks encountered during collection are rejected by default; `--follow-symlinks` opts into normal filesystem symlink-following behavior and should only be used with trusted input trees that remain stable during the write.

## Performance

The stateless `ArchiveTool` facade opens archives once per high-level operation; use `ArchiveTool::open` / `OpenArchive` for repeated list/read/extract calls against the same archive. Single-file extraction and `extract-all` stream entry payloads into their output writer through `dream_archive` instead of first materializing whole files in `dream-archivetool`. `extract-all` checks the destination before decoding entry payloads, so `--skip-existing` avoids reading skipped files. Directory inputs for `create` and `add` are stored relative to each directory root; the root directory name itself is not preserved.

Lua report and plan functions materialize their result tables. `verify`, `diff`, `plan_extract_all`, and large selected-entry plans can therefore allocate a lot of Lua objects and put pressure on LuaJIT's GC. Batch extraction opens the archive once for the batch, but selected entry arrays are validated and copied at the Rust boundary. Prefer `extract_many` / `extract_many_by_path_hex` over looping single-entry extraction; prefer `dream_archive` primitives when you only need low-level listing or payload reads.

Archive creation and update preflight archive paths and format policy before adding deferred payload sources to the backend builders. TES3, TES4, and BA2 GNRL creation pass filesystem paths to `dream_archive` rather than preloading payloads in `dream-archivetool`. `add` preserves unchanged TES3, TES4, BA2 GNRL, and BA2 DX10 entries from the source archive through deferred archive-entry sources instead of decoding them into `dream-archivetool` memory. BA2 DX10 still has to parse DDS texture data for new or replaced files, but preserved texture entries are copied as native BA2 chunks.

## Format Notes

- `add` writes a new archive and preserves source archive settings only where `dream_archive` currently exposes them; BA2 version variants such as Starfield v2 and Fallout 4 next-gen v8 are preserved. Archives with entries that do not have recoverable path names, including TES4 hash-only archives, are rejected rather than rewritten lossy.
- Format-specific `create` options are rejected with other formats: `--tes4-version` only applies to `--format tes4`, while `--ba2-kind` and `--ba2-version` only apply to `--format ba2`.
- `list --json` includes `path` as a lossy display string and `path_bytes_hex` as the normalized archive-path lookup bytes for scripts. Use `extract --entry-hex HEX` to feed that lookup key back into the CLI; do not treat it as raw unique archive-name identity.
- `create --format ba2 --ba2-kind gnrl` is the general-purpose BA2 mode and accepts any file names.
- `create --format ba2 --ba2-kind dx10` only accepts `.dds` entries. This extension check is case-insensitive; the underlying writer may still reject invalid DDS data.
- `create --format ba2 --ba2-kind gnmf` is accepted by argument parsing but rejected before writing. GNMF writing requires console texture swizzle semantics that `dream_archive` intentionally does not implement yet.
- Newly-created BA2 archives are written with string tables enabled so entries can be listed and extracted by path later.
- TES4 BSA creation defaults to miscellaneous archive type flags.

## Development

```bash
cargo fmt --check
cargo test --workspace
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo test --workspace --no-default-features --features standalone-lua
cargo check --no-default-features
cargo check --no-default-features --features standalone-lua
cargo clippy --workspace --all-targets --all-features -- -W clippy::pedantic -D warnings
cargo clippy --workspace --all-targets --no-default-features --features standalone-lua -- -W clippy::pedantic -D warnings
cargo build --release
cargo build --release --no-default-features
cargo bench --bench archive_ops
```

Use `cargo bench --bench archive_ops` to profile generated synthetic archives for listing, single-entry lookup, opened-archive reads, whole-archive extraction, skip-existing extraction, large-payload streaming, many-entry scans, creation, update, verify, and diff paths. The bench binary installs a tracking allocator and prints peak allocator deltas for one representative run of each operation before Criterion times it. Those numbers are not a replacement for OS-level RSS measurements, but they catch surprise heap growth inside the policy layer. On Linux, `/usr/bin/time -v cargo bench --bench archive_ops` is still useful for checking peak resident memory while tuning large archive operations.

## License

GPL-3.0-or-later. See `LICENSE`.

## Support

Has `dream_archivetool` been useful to you?

If so, please consider [amplifying the signal](https://ko-fi.com/magicaldave) through my ko-fi. 

Thank you for using `dream_archivetool`.
