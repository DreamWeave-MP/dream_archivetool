# dream_archivetool

`dream_archivetool` is a Rust CLI and library for inspecting, extracting, creating, and updating Bethesda BSA and BA2 archives.

The tool is intentionally designed as a reusable library first, with a thin CLI wrapper. It uses the `dream_archive` crate for archive format support. See [`docs/architecture.md`](docs/architecture.md) for subsystem ownership, compatibility contracts, and Luau handle semantics.

## Goals

- List archive contents in human-readable or JSON form.
- Extract single files or whole archives safely.
- Create TES3 BSA, TES4 BSA, and BA2/Starfield BA2 archives.
- Add or update archive entries by rewriting the archive, optionally to a separate output.
- Keep CLI parsing dependencies behind the default `cli` feature so GUI/library consumers can opt out.
- Expose an optional Luau embedding API behind the `lua` feature.

## Usage

```bash
dream_archivetool info archive.bsa
dream_archivetool info --json archive.bsa
dream_archivetool list archive.bsa
dream_archivetool list --long archive.bsa
dream_archivetool list --json archive.bsa
dream_archivetool verify archive.bsa --read-payloads --json
dream_archivetool diff old.bsa new.bsa --hash --json
dream_archivetool extract archive.bsa textures/example.dds --output out/
dream_archivetool extract archive.bsa textures/example.dds --stdout > example.dds
dream_archivetool extract archive.bsa --entry-hex 74657874757265732f6578616d706c652e646473 --stdout > example.dds
dream_archivetool extract-all archive.bsa --output out/ --dry-run
dream_archivetool extract-all archive.bsa --output out/
# extract/extract-all default to the current directory when --output is omitted
dream_archivetool create out.bsa input_dir/ --format tes3
dream_archivetool create out.bsa input_dir/ --format tes4 --tes4-version oblivion
dream_archivetool create out.bsa input_dir/ --format tes4 --compress
dream_archivetool create out.ba2 input_dir/ --format ba2 --ba2-kind gnrl
dream_archivetool create out.ba2 input_dir/ --format ba2 --ba2-kind gnrl --compress
dream_archivetool create out.bsa input_dir/ --format tes3 --follow-symlinks
dream_archivetool add base.bsa new_file.txt
dream_archivetool add base.bsa new_file.txt --output updated.bsa
dream_archivetool add base.bsa new_dir/ --output updated.bsa --dry-run
dream_archivetool --generate-completion bash > dream_archivetool.bash
dream_archivetool --generate-manpage > dream_archivetool.1
```

## CLI Contracts

Running `dream_archivetool` without a subcommand intentionally prints top-level help and exits successfully. Argument parsing errors still use clap's nonzero misuse exit code, and runtime archive/file failures return a runtime error.

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
the mutating commands use, but stop before writing output. Existing extraction targets do not make a plan fail: each entry reports `conflict` (default fail policy), `overwrite`, or `skip`, and only the real extraction errors on conflicts. The `--json` flag is accepted with dry-run commands for consistency, but dry-run output is already JSON. Add plans use stable report order grouped
by action; they are not a physical archive-order manifest.

## Library

The crate exposes `ArchiveTool` and option structs for reuse by other applications. Public report/plan DTOs and evolvable enums are marked non-exhaustive so new fields or variants can be added without pretending today's archive formats are the end of history. Option structs remain literal-friendly; prefer `..Default::default()` in application code so future major-version additions are easier to adopt.

GUI or embedding projects that do not need the command-line interface should disable default features to avoid pulling in `clap`, completion generation, manpage generation, and `serde_json` dependencies:

```toml
[dependencies]
dream_archivetool = { version = "0.2", default-features = false }
```

The `cli` feature is enabled by default for building the `dream_archivetool` binary. The binary target requires `cli`, so `cargo build --no-default-features` builds the library without producing a nonfunctional CLI stub. Add `features = ["luau"]` if the embedding API is needed: it enables this crate's l3i extension and `dream_archive`'s (`lua` is the old name of the same feature). The intended Luau stack is `dream_archive` for archive mechanics and `dream_archivetool` for filesystem/rewrite policy, composed into one l3i runtime plan by the host; `dream_archive` is re-exported as `dream_archivetool::dream_archive` so downstream users get the same crate and feature set this policy layer was compiled against.

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

## Luau

Enable the `luau` feature to get the bindings as an [l3i](https://github.com/DreamWeave-MP/l3i)
extension: `dream.archivetool`, module `@dream/archivetool`, requiring `dream.archive`. The
crate never creates a Luau VM; the host composes both extensions into one `RuntimePlan` and
decides whether the modules are also globals (the conventional names are still `dreamArchive`
and `dreamArchivetool`). l3i's toolchain policy (clang, lld, cross-language thin LTO) applies to
anything that builds the `luau` feature; copy l3i's `.cargo/config.toml` as this repository does.

```rust,no_run
use l3i::Runtime;
use l3i::extension::{RuntimePlan, RuntimePolicy};

# fn main() -> l3i::Result<()> {
let plan = RuntimePlan::builder()
    .policy(
        RuntimePolicy::new()
            .compat_global("@dream/archive", "dreamArchive")
            .compat_global("@dream/archivetool", "dreamArchivetool"),
    )
    .extension(dream_archive::luau::ArchiveExtension)
    .extension(dream_archivetool::luau::ArchivetoolExtension)
    .finalize()?;
let runtime = Runtime::from_plan(&plan)?;
runtime.exec(r#"
    local archive = dreamArchive.openPath("Morrowind.bsa")
    local plan = archive:planExtractAll({ output = "out" })
    for _, row in plan.entries do print(row.action, row.path, row.target) end
    print(dreamArchivetool.info("Morrowind.bsa").fileCount)
"#)?;
# Ok(())
# }
```

As of 0.3.0 the policy methods live on `dream.archive.Archive` itself: the extension augments
the type the `dream.archive` extension owns (`verify`, `diff`, `extract`, `extractMany`,
`planExtract`, `extractByPathHex`, `extractManyByPathHex`, `planExtractByPathHex`,
`extractAll`, `planExtractAll`, `toolInfo`), so every archive from `dreamArchive.open*` has them
and there is no second wrapper type or registration order to get right. The module keeps the
host-path functions (`info`, `verify`, `diff`, `extract`, `extractHex`, `extractByPathHex`,
`extractMany`, `extractManyByPathHex`, `planExtract`, `planExtractByPathHex`, `extractAll`,
`planExtractAll`, `create`, `planCreate`, `add`, `planAdd`), which open the archive for that one
operation. Creation and rewrite stay host-path operations; there are intentionally no
`archive:add`, `archive:create`, or their plans as methods. Every member is typed: the plan
renders `.d.luau` definitions and `plan.check_definitions()` is the gate this crate's tests run.

Luau string boundaries are deliberately split. Filesystem paths (`archive`, `output`, `input`,
and the paths in `inputs`) are UTF-8 host paths. Archive entry arguments are byte strings, entry
handles from `archive:entries()`, or a whole entries view (`archive:extractMany(archive:entries(),
opts)`); the `*ByPathHex` forms take the serialized `pathBytesHex` lookup key that reports carry.
Hash-only or unnameable entries are refused as extraction targets, and a plan or batch that names
a member the archive does not have fails before anything is written. Display `path` fields are
for people; `pathBytesHex` is the stable normalized lookup key.

Reports and plans are userdata: scalar fields are read as properties (`report.fileCount`,
`plan.output`), row lists are sequence views of row handles (`#plan.entries`, `plan.entries[i]`,
`for _, row in plan.entries`), and `:toTable()` gives the nested table 0.2 returned. Wide sizes
(`size`, `compressedSize`) are Luau integers, payload fingerprints (`payloadFingerprint`) are
Luau integers carrying all 64 FNV-1a bits (compare with `==`), and counts (`fileCount`, `files`,
`extracted`, `added`, ...) stay plain numbers. Option tables are strict: an unknown or misspelt
key is an error naming it and the known keys.

```luau
local report = archive:diff(other, { fingerprintPayloads = true })
for _, change in report.changed do
    if change.old.payloadFingerprint ~= change.new.payloadFingerprint then
        print(change.path, change.old.size, change.new.size)
    end
end
local issues = archive:verify().duplicateNormalizedPaths
print(#issues, issues[1] and issues[1].pathBytesHex)
```

### Breaking changes from 0.2

- Rust: `dream_archivetool::lua` and the `mlua` types are gone; `dream_archivetool::luau`
  exports `ArchivetoolExtension`, the report and plan userdata types, and the module constants.
  `create_dream_archive_module` / `register_dream_archive_methods` have no equivalent: composing
  the two extensions into a plan is the whole registration. `lua` is an alias of the `luau`
  feature.
- Reports and plans (`verify`, `diff`, `planExtract*`, `planCreate`, `planAdd`) are userdata
  with sequence views, not tables; `ipairs`, `pairs`, `#` on the report itself, and
  `table.insert` do not apply. `:toTable()` restores the old shape.
- `size` and `compressedSize` are integers, not decimal strings; `payloadFingerprint` is an
  integer, not a hex string (in `:toTable()` too).
- Unknown option keys report as `<context>: unknown option 'key'; known options are ...`
  instead of `<context>: unknown option key: key`; option type errors name the field
  (`add.output: ...`).
- `planExtract`, `planExtractByPathHex`, `extractMany`, and `extract` fail with
  "archive entry not found" for a member the archive lacks instead of planning it or writing
  part of the batch first.

### Performance

`benches/luau_boundary.rs` runs frozen scripts against a 2000-member TES3 archive opened
through `dreamArchive.openPath`; mean per script call, 0.2.1 with mlua against 0.3.0 with l3i on
the same machine:

| scenario | 0.2.1 (mlua) | 0.3.0 (l3i) |
| --- | --- | --- |
| `planExtract` of 2000 paths | 10.0 ms | 6.5 ms |
| `planExtractAll().entries` iterated (2000 rows) | 6.9 ms | 6.8 ms |
| `planExtractAll().entries[i]` indexed (2000 rows) | 6.8 ms | 6.2 ms |
| `extractMany` of 256 members to disk | 5.8 ms | 5.5 ms |
| `verify()` report (2000 members) | 2.0 ms | 1.0 ms |
| `diff(other, { fingerprintPayloads = true })` | 6.5 ms | 2.5 ms |
| `diff` rows iterated | 2.6 ms | 1.6 ms |
| `toolInfo()` | 1.2 µs | 1.0 µs |

The plan and batch scenarios are bound by the filesystem (one `stat` per planned target, one
file per extracted member), so the report and diff scenarios show the boundary's share: no row
tables are built until a script asks for one.

## Safety

Extraction rejects absolute paths, `..` components, NUL bytes, and colon-containing components before writing files. Existing targets fail by default; pass `--overwrite` or `--skip-existing` to choose another policy. `extract` and `extract-all` write under the current directory when `--output` is omitted. These checks validate archive path syntax; they are not an `openat`-style filesystem jail, so extract into an output tree whose pre-existing directories and symlinks you trust.

Archive creation and update write to a temporary file in the output directory, then rename it into place after a successful write. For `add`, omitting `--output` replaces the source archive only after the full rewritten archive has been produced; this is not patch-in-place mutation. Failed writes should not clobber an existing output archive. Input symlinks encountered during collection are rejected by default; `--follow-symlinks` opts into normal filesystem symlink-following behavior and should only be used with trusted input trees that remain stable during the write.

## Performance

The stateless `ArchiveTool` facade opens archives once per high-level operation; use `ArchiveTool::open` / `OpenArchive` for repeated list/read/extract calls against the same archive. Single-file extraction and `extract-all` stream entry payloads into their output writer through `dream_archive` instead of first materializing whole files in `dream_archivetool`. `extract-all` checks the destination before decoding entry payloads, so `--skip-existing` avoids reading skipped files. Directory inputs for `create` and `add` are stored relative to each directory root; the root directory name itself is not preserved.

Lua report and plan functions materialize their result tables. `verify`, `diff`, `planExtractAll`, and large selected-entry plans can therefore allocate a lot of Lua objects and put pressure on Luau's GC. Batch extraction opens the archive once for the batch, but selected entry arrays are validated and copied at the Rust boundary. Prefer `extractMany` / `extractManyByPathHex` over looping single-entry extraction; prefer `dream_archive` primitives when you only need low-level listing or payload reads.

Archive creation and update preflight archive paths and format policy before adding deferred payload sources to the backend builders. TES3, TES4, and BA2 GNRL creation pass filesystem paths to `dream_archive` rather than preloading payloads in `dream_archivetool`. `add` preserves unchanged TES3, TES4, BA2 GNRL, and BA2 DX10 entries from the source archive through deferred archive-entry sources instead of decoding them into `dream_archivetool` memory. BA2 DX10 still has to parse DDS texture data for new or replaced files, but preserved texture entries are copied as native BA2 chunks.

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
cargo test --workspace --no-default-features --features luau
cargo test --features luau-analysis --test luau_api
cargo check --no-default-features
cargo clippy --workspace --all-targets --all-features -- -W clippy::pedantic -D warnings
cargo build --release
cargo build --release --no-default-features
cargo bench --bench archive_ops
cargo bench --bench luau_boundary --features luau
```

The `luau` feature builds l3i, which needs clang++, lld, and the cross-language LTO flags in
`.cargo/config.toml` (copied from l3i). The `luau-analysis` feature (`luau` plus `l3i/analysis`)
adds the definitions gate and the strict script to the Luau tests; `--all-features` includes it,
and plain `--features luau` never builds the analysis frontend.

Use `cargo bench --bench archive_ops` to profile generated synthetic archives for listing, single-entry lookup, opened-archive reads, whole-archive extraction, skip-existing extraction, large-payload streaming, many-entry scans, creation, update, verify, and diff paths. The bench binary installs a tracking allocator and prints peak allocator deltas for one representative run of each operation before Criterion times it. Those numbers are not a replacement for OS-level RSS measurements, but they catch surprise heap growth inside the policy layer. On Linux, `/usr/bin/time -v cargo bench --bench archive_ops` is still useful for checking peak resident memory while tuning large archive operations.

## License

GPL-3.0-or-later. See `LICENSE`.

## Support

Has `dream_archivetool` been useful to you?

If so, please consider [amplifying the signal](https://ko-fi.com/magicaldave) through my ko-fi. 

Thank you for using `dream_archivetool`.
