+++
title = "ArchiveTool and OpenArchive"
description = "Every ArchiveTool function, by archive path, and every OpenArchive method, on an archive opened once."
weight = 10

[extra]
kind = "api"
+++

## ArchiveTool

{{ api_signature(value="struct ArchiveTool") }}

A unit struct holding nothing; its associated functions are the whole library. Each one opens
the archive it is given, does one operation, and drops the archive. `Debug`, `Default`, `Clone`,
`Copy`.

The same operations are free functions in the `entry`, `extract`, `create`, `verify`, `diff` and
`format` modules, taking `&Path`; `ArchiveTool` takes `impl AsRef<Path>` and is the one to use.
Each page lists its module's functions.

### Opening and inspecting

{{ api_signature(value="fn open(path: impl AsRef<Path>) -> Result<OpenArchive>") }}

Opens the archive and keeps it: see [OpenArchive](#openarchive).

{{ api_signature(value="fn guess_format(path: impl AsRef<Path>) -> Result<ArchiveFormat>") }}

Reads only the header. `ArchiveError::UnknownFormat` when it is not a BSA or BA2, and
`ArchiveError::Archive` when the file cannot be opened or read.

{{ api_signature(value="fn info(path: impl AsRef<Path>) -> Result<ArchiveInfo>") }}

Format, counts, rewritability and header facts: [`ArchiveInfo`](@/docs/api/inspecting.md#archiveinfo).

{{ api_signature(value="fn list(path: impl AsRef<Path>) -> Result<Vec<ArchiveEntry>>") }}

Every named entry, in the archive's order: [`ArchiveEntry`](@/docs/api/inspecting.md#archiveentry).

{{ api_signature(value="fn verify(path: impl AsRef<Path>, options: &VerifyOptions) -> Result<VerifyReport>") }}

{{ api_signature(value="fn diff(old: impl AsRef<Path>, new: impl AsRef<Path>, options: &DiffOptions) -> Result<DiffReport>") }}

What [`verify`](@/docs/inspecting.md#verify) and [`diff`](@/docs/inspecting.md#diff) print.
The reports are on [Inspecting](@/docs/api/inspecting.md#verifyreport).

### Reading entries

{{ api_signature(value="fn read_entry(path: impl AsRef<Path>, entry: &str) -> Result<Vec<u8>>") }}

{{ api_signature(value="fn read_entry_by_path_bytes(path: impl AsRef<Path>, entry: &[u8]) -> Result<Vec<u8>>") }}

One entry's bytes, decompressed, in memory. The name is normalized before the lookup, so any
spelling finds it; `ArchiveError::EntryNotFound` when none does. These read; they do not apply
the [unsafe name](@/docs/paths.md#unsafe-names) rules, which are about writing files.

{{ api_signature(value="fn extract_entry_to_writer(path: impl AsRef<Path>, entry: &str, out: &mut dyn Write) -> Result<u64>") }}

{{ api_signature(value="fn extract_entry_by_path_bytes_to_writer(path: impl AsRef<Path>, entry: &[u8], out: &mut dyn Write) -> Result<u64>") }}

The same, streamed into `out` as it is decompressed, never whole in memory. Returns the number of
bytes written. `extract --stdout` is this.

{{ api_signature(value="fn read_entry_by_path(path: impl AsRef<Path>, entry: &[u8]) -> Result<Vec<u8>>") }}

{{ api_signature(value="fn extract_entry_path_to_writer(path: impl AsRef<Path>, entry: &[u8], out: &mut dyn Write) -> Result<u64>") }}

Older names of `read_entry_by_path_bytes` and `extract_entry_by_path_bytes_to_writer`, kept so
existing code builds. Prefer the `_bytes` names.

### Extracting to disk

{{ api_signature(value="fn extract(path: impl AsRef<Path>, entry: &str, options: &ExtractOptions) -> Result<ExtractSummary>") }}

{{ api_signature(value="fn extract_by_path_bytes(path: impl AsRef<Path>, entry: &[u8], options: &ExtractOptions) -> Result<ExtractSummary>") }}

{{ api_signature(value="fn extract_by_path(path: impl AsRef<Path>, entry: &[u8], options: &ExtractOptions) -> Result<ExtractSummary>") }}

One entry to a file, as [`extract`](@/docs/extracting.md#one-file) does: the name is checked,
the entry must exist, and the target is written through a temporary file. `extract_by_path` is
the older name of `extract_by_path_bytes`.

{{ api_signature(value="fn extract_many_by_path_bytes(path: impl AsRef<Path>, entries: &[Vec<u8>], options: &ExtractOptions) -> Result<ExtractSummary>") }}

Several entries, opening the archive once. Before writing anything, it checks every name, that
every entry exists (`EntryNotFound` for the first that does not), that no two targets are the
same file, and under `OverwriteMode::Fail` that no target exists (`TargetExists`).

{{ api_signature(value="fn plan_extract_many_by_path_bytes(path: impl AsRef<Path>, entries: &[Vec<u8>], options: &ExtractOptions) -> Result<ExtractAllPlan>") }}

What `extract_many_by_path_bytes` would do, writing nothing. It fails for the same reasons
except existing targets, which it reports as `ExtractPlanAction::Conflict`.

{{ api_signature(value="fn extract_all(path: impl AsRef<Path>, options: &ExtractAllOptions) -> Result<ExtractSummary>") }}

{{ api_signature(value="fn plan_extract_all(path: impl AsRef<Path>, options: &ExtractAllOptions) -> Result<ExtractAllPlan>") }}

Every named entry, as [`extract-all`](@/docs/extracting.md#everything) does, and its plan. An
archive with unnamed entries is refused with `ArchiveError::Archive`.

### Creating and updating

{{ api_signature(value="fn create(output: impl AsRef<Path>, input: impl AsRef<Path>, options: &CreateOptions) -> Result<usize>") }}

{{ api_signature(value="fn plan_create(output: impl AsRef<Path>, input: impl AsRef<Path>, options: &CreateOptions) -> Result<CreatePlan>") }}

Packs a folder or file into a new archive, as [`create`](@/docs/creating.md#create) does, and
returns the number of entries written; and its plan. A file already at `output` is replaced only
when the new archive is complete.

{{ api_signature(value="fn add(path: impl AsRef<Path>, options: &AddOptions) -> Result<usize>") }}

{{ api_signature(value="fn plan_add(path: impl AsRef<Path>, options: &AddOptions) -> Result<AddPlan>") }}

Writes the archive again with `options.inputs` added or replacing entries, as
[`add`](@/docs/creating.md#add) does, and returns the number of entries in the new archive; and
its plan. An archive it cannot rewrite without loss is refused before anything is read from the
inputs.

## OpenArchive

{{ api_signature(value="struct OpenArchive") }}

An archive opened once, for any number of reads. Opening parses the whole index, which is most
of what a single read costs; an `OpenArchive` pays it once. It has no trait implementations.

```rust
use dream_archivetool::ArchiveTool;

fn main() -> dream_archivetool::Result<()> {
    let archive = ArchiveTool::open("Morrowind.bsa")?;
    let icons: Vec<_> = archive
        .list()?
        .into_iter()
        .filter(|entry| entry.path.starts_with("icons/"))
        .collect();
    let mut total = 0;
    for icon in &icons {
        total += archive.read_entry(&icon.path)?.len();
    }
    println!("{} icons, {total} bytes", icons.len());
    Ok(())
}
```

{{ api_signature(value="fn open(path: impl AsRef<Path>) -> Result<OpenArchive>") }}

The same as `ArchiveTool::open`.

{{ api_signature(value="fn info(&self) -> ArchiveInfo") }}

{{ api_signature(value="fn format(&self) -> ArchiveFormat") }}

{{ api_signature(value="fn file_count(&self) -> usize") }}

{{ api_signature(value="fn list(&self) -> Result<Vec<ArchiveEntry>>") }}

As the `ArchiveTool` functions, without opening anything. `info` reports the path the archive
was opened with. `file_count` counts every entry, named or not.

{{ api_signature(value="fn read_entry(&self, entry: &str) -> Result<Vec<u8>>") }}

{{ api_signature(value="fn read_entry_by_path_bytes(&self, entry: &[u8]) -> Result<Vec<u8>>") }}

{{ api_signature(value="fn extract_entry_to_writer(&self, entry: &str, out: &mut dyn Write) -> Result<u64>") }}

{{ api_signature(value="fn extract_entry_by_path_bytes_to_writer(&self, entry: &[u8], out: &mut dyn Write) -> Result<u64>") }}

{{ api_signature(value="fn read_entry_by_path(&self, entry: &[u8]) -> Result<Vec<u8>>") }}

{{ api_signature(value="fn extract_entry_path_to_writer(&self, entry: &[u8], out: &mut dyn Write) -> Result<u64>") }}

Reads, as above. The last two are the older names of the `_bytes` methods.

`OpenArchive` reads; it does not extract to disk, verify or compare. Those are `ArchiveTool`
functions, or, from Luau, [archive methods](@/docs/luau/archive.md) that run on an opened
archive.
