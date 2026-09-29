+++
title = "Extracting"
description = "ExtractOptions, ExtractAllOptions, OverwriteMode, ExtractSummary, the extraction plan and its rows, and the extract module's functions."
weight = 30

[extra]
kind = "api"
+++

[Extracting](@/docs/extracting.md) describes what extraction checks and how it writes. The
options here are what the command line's flags set.

## ExtractOptions

{{ api_signature(value="struct ExtractOptions") }}

For extracting chosen entries. `Debug`, `Clone`, `Default`.

| Field | Default | Is |
|---|---|---|
| `output: Option<PathBuf>` | `None` | The folder to write under; `None` is the current folder |
| `overwrite: OverwriteMode` | `Fail` | What to do with a target that exists |
| `preserve_paths: bool` | `true` | Keep the archive's folders; `false` writes the file name alone, as `--flat` does |
| `fsync: bool` | `false` | Flush each file, and on Unix the folders it created or changed |

## ExtractAllOptions

{{ api_signature(value="struct ExtractAllOptions") }}

For extracting everything: `output`, `overwrite` and `fsync`, as above, with the same defaults.
There is no `preserve_paths`. `Debug`, `Clone`, `Default`.

## OverwriteMode

{{ api_signature(value="enum OverwriteMode { Fail, Overwrite, Skip }") }}

| Variant | A target that exists |
|---|---|
| `Fail`, the default | Is an error, `ArchiveError::TargetExists`; a batch fails before writing anything |
| `Overwrite` | Is replaced |
| `Skip` | Is left alone, counted in `skipped`, and its entry is not read |

`Debug`, `Clone`, `Copy`, `Default`, `PartialEq`, `Eq`.

## ExtractSummary

{{ api_signature(value="struct ExtractSummary { pub extracted: usize, pub skipped: usize }") }}

What an extraction did: files written, and targets left alone under `Skip`. `#[non_exhaustive]`,
serializes as the command line's `--json` summary.

## ExtractAllPlan

{{ api_signature(value="struct ExtractAllPlan") }}

What an extraction would do, from `plan_extract_all` or `plan_extract_many_by_path_bytes`.
`#[non_exhaustive]`, `Serialize`, `Deserialize`.

| Field | Is |
|---|---|
| `operation: ExtractPlanOperation` | `ExtractAll`, or `Extract` for chosen entries |
| `archive: String` | The archive's path, for display |
| `output: String` | The output folder, for display: `.` when none was given |
| `entries: Vec<ExtractPlanEntry>` | One per entry, in the order given, or the archive's order |

{{ api_signature(value="struct ExtractPlanEntry") }}

| Field | Is |
|---|---|
| `action: ExtractPlanAction` | What would happen |
| `path: String` | The entry's normalized name, as text |
| `path_bytes_hex: String` | The same as a lookup key |
| `target: String` | The file it would write, for display |

{{ api_signature(value="enum ExtractPlanAction { Extract, Skip, Overwrite, Conflict }") }}

`Extract`: the target does not exist. `Overwrite` and `Skip`: it exists, and the mode says what
happens. `Conflict`: it exists under `Fail`, and the extraction would fail. Serialized in
kebab-case. `#[non_exhaustive]`.

{{ api_signature(value="enum ExtractPlanOperation { Extract, ExtractAll }") }}

Serialized as `"extract"` and `"extract-all"`. `#[non_exhaustive]`.

```rust
use dream_archivetool::{ArchiveTool, ExtractOptions, ExtractPlanAction};

fn main() -> dream_archivetool::Result<()> {
    let wanted = vec![
        b"icons/tx_goldicon.dds".to_vec(),
        b"Textures\\TX_BC_Moss.dds".to_vec(),
    ];
    let options = ExtractOptions {
        output: Some("picked".into()),
        ..ExtractOptions::default()
    };
    let plan = ArchiveTool::plan_extract_many_by_path_bytes("Morrowind.bsa", &wanted, &options)?;
    if plan.entries.iter().any(|row| row.action == ExtractPlanAction::Conflict) {
        println!("something is already there; nothing written");
        return Ok(());
    }
    let summary = ArchiveTool::extract_many_by_path_bytes("Morrowind.bsa", &wanted, &options)?;
    assert_eq!(summary.extracted, 2);
    Ok(())
}
```

## The extract module

The functions behind `ArchiveTool`'s, taking `&Path`:

{{ api_signature(value="fn read_entry_bytes(path: &Path, entry: &str) -> Result<Vec<u8>>") }}

{{ api_signature(value="fn read_entry_bytes_by_path(path: &Path, entry: &[u8]) -> Result<Vec<u8>>") }}

{{ api_signature(value="fn extract_entry_to_writer(path: &Path, entry: &str, out: &mut dyn Write) -> Result<u64>") }}

{{ api_signature(value="fn extract_entry_path_to_writer(path: &Path, entry: &[u8], out: &mut dyn Write) -> Result<u64>") }}

{{ api_signature(value="fn extract_entry(path: &Path, entry: &str, options: &ExtractOptions) -> Result<ExtractSummary>") }}

{{ api_signature(value="fn extract_entry_by_path(path: &Path, entry: &[u8], options: &ExtractOptions) -> Result<ExtractSummary>") }}

{{ api_signature(value="fn extract_entries_by_path(path: &Path, entries: &[Vec<u8>], options: &ExtractOptions) -> Result<ExtractSummary>") }}

{{ api_signature(value="fn plan_extract_entries_by_path(path: &Path, entries: &[Vec<u8>], options: &ExtractOptions) -> Result<ExtractAllPlan>") }}

{{ api_signature(value="fn extract_all(path: &Path, options: &ExtractAllOptions) -> Result<ExtractSummary>") }}

{{ api_signature(value="fn plan_extract_all(path: &Path, options: &ExtractAllOptions) -> Result<ExtractAllPlan>") }}

| Function | Is `ArchiveTool::` |
|---|---|
| `read_entry_bytes`, `read_entry_bytes_by_path` | `read_entry`, `read_entry_by_path_bytes` |
| `extract_entry_to_writer`, `extract_entry_path_to_writer` | `extract_entry_to_writer`, `extract_entry_by_path_bytes_to_writer` |
| `extract_entry`, `extract_entry_by_path` | `extract`, `extract_by_path_bytes` |
| `extract_entries_by_path`, `plan_extract_entries_by_path` | `extract_many_by_path_bytes`, `plan_extract_many_by_path_bytes` |
| `extract_all`, `plan_extract_all` | `extract_all`, `plan_extract_all` |
