+++
title = "Rust API"
description = "Every public type and function in the dream_archivetool library: ArchiveTool and OpenArchive, the options, the reports and plans, the path helpers and the error."
template = "docs/section.html"
page_template = "docs/page.html"
sort_by = "weight"
weight = 90

[extra]
kind = "api"
hide_child_cards = true
+++

The crate `dream_archivetool` is what the command line runs, as a library. `ArchiveTool` is the
way in; everything else is its options and results. The common types are exported from the crate
root, and each module's page names the rest.

```toml
[dependencies]
dream_archivetool = { version = "1", default-features = false }
```

`default-features = false` leaves out the command line and its dependencies.

```rust
use dream_archivetool::{ArchiveTool, ExtractAllOptions, OverwriteMode};

fn main() -> dream_archivetool::Result<()> {
    let info = ArchiveTool::info("Morrowind.bsa")?;
    println!("{:?}: {} files, rewritable: {}", info.format, info.file_count, info.rewritable);

    let options = ExtractAllOptions {
        output: Some("out".into()),
        overwrite: OverwriteMode::Skip,
        ..ExtractAllOptions::default()
    };
    let plan = ArchiveTool::plan_extract_all("Morrowind.bsa", &options)?;
    println!("{} files planned", plan.entries.len());

    let summary = ArchiveTool::extract_all("Morrowind.bsa", &options)?;
    println!("extracted {}, skipped {}", summary.extracted, summary.skipped);
    Ok(())
}
```

| Page | Covers |
|---|---|
| [ArchiveTool and OpenArchive](@/docs/api/archive-tool.md) | Every operation, by path or on an archive opened once |
| [Inspecting](@/docs/api/inspecting.md) | `ArchiveInfo`, `ArchiveEntry`, `ArchiveFormat`, the verify report and the diff report |
| [Extracting](@/docs/api/extracting.md) | The extraction options, `OverwriteMode`, the summary and the extraction plan |
| [Creating and updating](@/docs/api/creating.md) | `CreateOptions`, `AddOptions`, the format enums and the two plans |
| [Paths and errors](@/docs/api/paths-errors.md) | The `path_bytes_hex` helpers, `ArchiveError`, and the `dream_archive` re-export |
| [ArchivetoolExtension](@/docs/luau/extension.md) | The `luau` module, behind the `luau` feature |

The archive formats themselves are [dream_archive](https://DreamWeave-MP.github.io/dream_archive/)'s,
re-exported as `dream_archivetool::dream_archive` for code that needs to go below this crate.

## Features

| Feature | Default | Adds to the library |
|---|---|---|
| `cli` | Yes | Nothing: it builds the program |
| `luau` | No | The `luau` module: [`ArchivetoolExtension`](@/docs/luau/extension.md), and dream_archive's `luau` feature. `lua` is the old name |
| `luau-analysis` | No | `luau` plus l3i's type checker, for this crate's typed tests |

## Conventions

- **Host paths** are anything `AsRef<Path>`: archives, output folders, inputs. The results
  report them with `Path::display`.
- **Archive entries** are named by `&str`, or by bytes in the `_bytes` functions, in any
  spelling; lookups [normalize](@/docs/paths.md) them. Results carry each entry's normalized name
  as display text in `path` and as its lookup key in `path_bytes_hex`.
- **Options** are plain structs with public fields and a `Default`. Build them with
  `..Default::default()`, so that a field added in a later release does not break your code.
- **Results** are `#[non_exhaustive]`: read their fields, match their enums with a `_` arm, and
  let the crate construct them. They serialize with serde to exactly the
  [JSON](@/docs/json.md) the command line prints.
- **Errors** are one type, [`ArchiveError`](@/docs/api/paths-errors.md#archiveerror), with
  `Result<T>` as its alias.
- **Nothing is cached.** Each `ArchiveTool` function opens the archive it names and drops it on
  return; [`OpenArchive`](@/docs/api/archive-tool.md#openarchive) keeps one open.
