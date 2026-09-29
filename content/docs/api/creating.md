+++
title = "Creating and updating"
description = "CreateOptions, AddOptions, Tes4Version, Ba2ArchiveKind, Ba2Version, CreatePlan, AddPlan and their rows, and the create module's functions."
weight = 40

[extra]
kind = "api"
+++

[Creating and updating](@/docs/creating.md) describes what `create` and `add` check, write and
refuse, and [Formats](@/docs/formats.md) what each choice writes.

## CreateOptions

{{ api_signature(value="struct CreateOptions") }}

`Debug`, `Clone`, `Default`.

| Field | Default | Is |
|---|---|---|
| `format: ArchiveFormat` | `Tes3` | The archive family to write |
| `tes4_version: Tes4Version` | `Oblivion` | Used with `Tes4` |
| `ba2_kind: Ba2ArchiveKind` | `Gnrl` | Used with `Ba2` |
| `ba2_version: Ba2Version` | `Fallout4` | Used with `Ba2` |
| `compress: bool` | `false` | Compress every file. An error with `Tes3` |
| `fsync: bool` | `false` | Flush the archive, and on Unix its folder, before returning |
| `follow_symlinks: bool` | `false` | Pack what symbolic links point to, instead of refusing them |

Unlike the command line, the library does not refuse a version or kind for another format: it is
ignored.

## AddOptions

{{ api_signature(value="struct AddOptions") }}

`Debug`, `Clone`, `Default`.

| Field | Default | Is |
|---|---|---|
| `inputs: Vec<PathBuf>` | empty | Folders and files to add; at least one, or `add` fails with `no input files supplied` |
| `output: Option<PathBuf>` | `None` | Where to write the new archive; `None` replaces the source. Not the source itself. A bare file name that does not exist yet fails with an I/O error: give it a folder, as `./new.bsa` |
| `fsync: bool` | `false` | Flush the archive, and on Unix its folder, before returning |
| `follow_symlinks: bool` | `false` | Pack what symbolic links point to, instead of refusing them |

## Format enums

All three are `Copy`, `#[non_exhaustive]`, and serialized in kebab-case.

{{ api_signature(value="enum Tes4Version { Oblivion, Fallout3, Skyrim, SkyrimSe }") }}

Versions 103, 104, 104 and 105. `Fallout3` and `Skyrim` write the same version.

{{ api_signature(value="enum Ba2ArchiveKind { Gnrl, Dx10, Gnmf }") }}

General files; DirectX textures, `.dds` only; console textures, which creating or updating
refuses.

{{ api_signature(value="enum Ba2Version { Fallout4, Starfield, Fallout4NextGen }") }}

Versions 1, 2 and 8.

## CreatePlan

{{ api_signature(value="struct CreatePlan") }}

What `plan_create` returns. `#[non_exhaustive]`, `Serialize`, `Deserialize`.

| Field | Is |
|---|---|
| `operation: ArchivePlanOperation` | `Create` |
| `format: ArchiveFormat` | What would be written |
| `output: String` | The archive's path, for display |
| `files: usize` | How many entries it would hold |
| `entries: Vec<ArchivePlanEntry>` | One `Add` row per entry, in name order |

## AddPlan

{{ api_signature(value="struct AddPlan") }}

What `plan_add` returns. `#[non_exhaustive]`, `Serialize`, `Deserialize`.

| Field | Is |
|---|---|
| `operation: ArchivePlanOperation` | `Add` |
| `archive: String`, `output: String` | The source archive, and where the new one would go |
| `format: ArchiveFormat` | The source's format, which the new archive keeps |
| `files: usize` | How many entries the new archive would hold |
| `added: usize`, `replaced: usize`, `preserved: usize` | How many are new, replace an entry, or are kept |
| `entries: Vec<ArchivePlanEntry>` | Kept entries in name order, then the inputs in name order |

The order is a report's, not the order entries will have in the archive.

{{ api_signature(value="struct ArchivePlanEntry") }}

| Field | Is |
|---|---|
| `action: ArchivePlanAction` | `Add`, `Replace` or `Preserve` |
| `source: Option<String>` | The input file, for display; `None` for a kept entry |
| `path: String`, `path_bytes_hex: String` | Its normalized name, as text and as a key |
| `size: Option<u64>` | The input file's size; `None` for a kept entry |

{{ api_signature(value="enum ArchivePlanAction { Add, Replace, Preserve }") }}

{{ api_signature(value="enum ArchivePlanOperation { Create, Add }") }}

Both `Copy`, `#[non_exhaustive]`, serialized in kebab-case.

```rust
use dream_archivetool::{AddOptions, ArchiveTool};

fn main() -> dream_archivetool::Result<()> {
    let options = AddOptions {
        inputs: vec!["Update".into()],
        output: Some("./MyMod-2.bsa".into()),
        ..AddOptions::default()
    };
    let plan = ArchiveTool::plan_add("MyMod.bsa", &options)?;
    println!(
        "{} files: {} added, {} replaced, {} kept",
        plan.files, plan.added, plan.replaced, plan.preserved
    );
    let written = ArchiveTool::add("MyMod.bsa", &options)?;
    assert_eq!(written, plan.files);
    Ok(())
}
```

## The create module

The functions behind `ArchiveTool`'s, taking `&Path`:

{{ api_signature(value="fn create_archive(output: &Path, input: &Path, options: &CreateOptions) -> Result<usize>") }}

{{ api_signature(value="fn plan_create_archive(output: &Path, input: &Path, options: &CreateOptions) -> Result<CreatePlan>") }}

{{ api_signature(value="fn add_to_archive(archive_path: &Path, options: &AddOptions) -> Result<usize>") }}

{{ api_signature(value="fn plan_add_to_archive(archive_path: &Path, options: &AddOptions) -> Result<AddPlan>") }}

`ArchiveTool::create`, `plan_create`, `add` and `plan_add`, in that order. The plan types and
their enums are also exported from `create`.
