+++
title = "Inspecting"
description = "ArchiveInfo, Tes4Info, Ba2Info, ArchiveEntry, ArchiveFormat, the verify options and report, the diff options and report, and their module functions."
weight = 20

[extra]
kind = "api"
+++

Every result here is `#[non_exhaustive]`, `Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize` and
`Deserialize`, and serializes to the JSON on [JSON output](@/docs/json.md).

## ArchiveFormat

{{ api_signature(value="enum ArchiveFormat { Tes3, Tes4, Ba2 }") }}

The archive's family: Morrowind's BSA, the TES4 BSAs from Oblivion to Skyrim Special Edition,
and BA2. `Copy`; serialized as `"tes3"`, `"tes4"` and `"ba2"`. Non-exhaustive: match it with a
`_` arm.

{{ api_signature(value="fn format::guess_format(path: &Path) -> Result<ArchiveFormat>") }}

What `ArchiveTool::guess_format` calls.

## ArchiveInfo

{{ api_signature(value="struct ArchiveInfo") }}

What `ArchiveTool::info` and `OpenArchive::info` return.

| Field | Is |
|---|---|
| `path: String` | The path the archive was opened with, for display |
| `format: ArchiveFormat` | Its family |
| `file_count: usize` | Every entry, named or not |
| `named_entry_count: usize` | Entries with a file name |
| `has_unnameable_entries: bool` | Whether any entry has only a hash |
| `rewritable: bool` | Whether `add` would rewrite it |
| `rewrite_blocker: Option<String>` | Why not, when `rewritable` is false. Left out of JSON when `None` |
| `tes4: Option<Tes4Info>` | For a TES4 archive |
| `ba2: Option<Ba2Info>` | For a BA2 |

{{ api_signature(value="struct Tes4Info") }}

| Field | Is |
|---|---|
| `version: String` | `"v103"`, `"v104"` or `"v105"` |
| `archive_types: String`, `archive_types_bits: u16` | The content-type bits, as dream_archive prints them and as a number |
| `archive_flags: Vec<String>` | The known flags that are set: `"directory-strings"`, `"file-strings"`, `"compressed"`, `"embedded-file-names"` |
| `archive_flags_bits: u32` | Every flag bit |
| `unsupported_archive_flags_bits: u32` | The set bits outside those four; not zero blocks a rewrite |
| `name_mode: String` | `"strings"`, `"embedded"`, `"strings-and-embedded"` or `"hash-only"` |

{{ api_signature(value="struct Ba2Info") }}

| Field | Is |
|---|---|
| `version: String` | `"v1"`, `"v2"`, `"v3"`, `"v7"` or `"v8"` |
| `payload_format: String` | `"gnrl"`, `"dx10"` or `"gnmf"` |
| `compression_format: String` | `"zip"` or `"lz4"`: the method, not whether anything is compressed |
| `strings: bool` | Whether it has a name table |

## ArchiveEntry

{{ api_signature(value="struct ArchiveEntry") }}

One named entry, from `list`.

| Field | Is |
|---|---|
| `path: String` | The normalized name, as text: bytes that are not UTF-8 become U+FFFD |
| `path_bytes_hex: String` | The normalized name's bytes as lowercase hex: the lookup key |
| `size: Option<u64>` | Its size, where the format records one; `None` for TES4 |
| `compressed_size: Option<u64>` | Its stored size, when it is compressed |

`path_bytes_hex` is `#[serde(default)]`, so JSON saved without it still deserializes.

{{ api_signature(value="fn entry::list_entries(path: &Path) -> Result<Vec<ArchiveEntry>>") }}

What `ArchiveTool::list` calls.

## Verifying

{{ api_signature(value="struct VerifyOptions { pub read_payloads: bool }") }}

`read_payloads` reads every named entry to the end, as `--read-payloads` does. `Copy`,
`Default`, `PartialEq`, `Eq`.

{{ api_signature(value="fn verify::verify_archive(path: &Path, options: &VerifyOptions) -> Result<VerifyReport>") }}

What `ArchiveTool::verify` calls. An entry that cannot be read is an error; everything else is in
the report.

### VerifyReport

{{ api_signature(value="struct VerifyReport") }}

| Field | Is |
|---|---|
| `path: String`, `format: ArchiveFormat` | As in `ArchiveInfo` |
| `file_count: usize`, `named_entry_count: usize` | As in `ArchiveInfo` |
| `unnameable_entries: usize` | Entries with only a hash |
| `rewritable: bool`, `rewrite_blocker: Option<String>` | As in `ArchiveInfo` |
| `duplicate_normalized_paths: Vec<VerifyPathIssue>` | One for each entry whose normalized name an earlier entry has |
| `unsafe_paths: Vec<VerifyPathIssue>` | One for each name extraction would refuse |
| `payloads_read: Option<usize>` | With `read_payloads`, how many entries were read; `None` without it, or when there are duplicates |
| `warnings: Vec<String>` | What `verify` prints as `warning:` lines |

{{ api_signature(value="struct VerifyPathIssue") }}

| Field | Is |
|---|---|
| `path: String` | The name as text |
| `path_bytes_hex: String` | The name's bytes as hex |
| `raw_path_bytes_hex: Option<String>` | For a duplicate, this entry's stored bytes |
| `colliding_raw_path_bytes_hex: Option<String>` | For a duplicate, the earlier entry's stored bytes |

For a duplicate, `path` and `path_bytes_hex` are the shared normalized key. For an unsafe path,
they are the name as stored, and the raw fields are `None`.

## Comparing

{{ api_signature(value="struct DiffOptions { pub fingerprint_payloads: bool }") }}

`fingerprint_payloads` reads both archives in full and compares a 64-bit FNV-1a fingerprint of
each entry, as `--hash` does. `Copy`, `Default`, `PartialEq`, `Eq`.

{{ api_signature(value="fn diff::diff_archives(old: &Path, new: &Path, options: &DiffOptions) -> Result<DiffReport>") }}

What `ArchiveTool::diff` calls. An archive with unnamed entries, or with two entries that share a
normalized name, is an error.

### DiffReport

{{ api_signature(value="struct DiffReport") }}

| Field | Is |
|---|---|
| `old: String`, `new: String` | The two paths, for display |
| `comparison: DiffComparison` | How entries were compared |
| `fingerprint_payloads: bool` | Whether fingerprints were computed |
| `added: Vec<DiffEntry>` | Entries only in `new`, in name order |
| `removed: Vec<DiffEntry>` | Entries only in `old`, in name order |
| `changed: Vec<DiffChange>` | Entries in both whose size, stored size or fingerprint differ |
| `unchanged: usize` | Entries in both that are the same |

{{ api_signature(value="enum DiffComparison { MetadataOnly, PayloadFingerprint }") }}

`MetadataOnly` compares sizes and stored sizes, and cannot see a change that keeps them.
`PayloadFingerprint` adds the fingerprint. `Copy`; serialized as `"metadata-only"` and
`"payload-fingerprint"`.

{{ api_signature(value="struct DiffEntry") }}

An entry on one side: `path: String`, `path_bytes_hex: String`, `size: Option<u64>`,
`compressed_size: Option<u64>`, and `payload_fingerprint: Option<String>`, 16 hex digits when
computed and left out of JSON when not.

{{ api_signature(value="struct DiffChange") }}

An entry on both sides: `path: String`, `path_bytes_hex: String`, and `old` and `new`, each a
`DiffEntryState`.

{{ api_signature(value="struct DiffEntryState") }}

One side of a change: `size: Option<u64>`, `compressed_size: Option<u64>` and
`payload_fingerprint: Option<String>`.

```rust
use dream_archivetool::{ArchiveTool, DiffOptions};

fn main() -> dream_archivetool::Result<()> {
    let report = ArchiveTool::diff(
        "MyMod.bsa",
        "MyMod-2.bsa",
        &DiffOptions { fingerprint_payloads: true },
    )?;
    for change in &report.changed {
        println!("{}: {:?} -> {:?}", change.path, change.old.size, change.new.size);
    }
    for entry in &report.added {
        println!("+ {}", entry.path);
    }
    Ok(())
}
```
