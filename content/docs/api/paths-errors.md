+++
title = "Paths and errors"
description = "normalize_archive_path_bytes, encode_archive_path_hex and decode_archive_path_hex; ArchiveError and Result; and the dream_archive re-export."
weight = 50

[extra]
kind = "api"
+++

## The path module

Helpers for the `path_bytes_hex` keys in reports and plans, for programs that store them and feed
them back. [Archive paths](@/docs/paths.md) explains the keys.

{{ api_signature(value="fn normalize_archive_path_bytes(path: impl AsRef<[u8]>) -> Vec<u8>") }}

The normalized form of an archive name: `\` to `/`, ASCII to lowercase, runs of separators to
one, leading separators gone. It checks nothing; lookups and extraction apply their own rules.

{{ api_signature(value="fn encode_archive_path_hex(path: &[u8]) -> String") }}

The bytes as lowercase hexadecimal, two digits each. It does not normalize: normalize first to
get a key that matches `path_bytes_hex`.

{{ api_signature(value="fn decode_archive_path_hex(hex: &str) -> Result<Vec<u8>>") }}

The bytes back, from upper- or lowercase digits. An odd number of digits, or a character that is
not one, is `ArchiveError::Archive`.

```rust
use dream_archivetool::{decode_archive_path_hex, encode_archive_path_hex, normalize_archive_path_bytes};

fn main() -> dream_archivetool::Result<()> {
    let key = encode_archive_path_hex(&normalize_archive_path_bytes(r"Icons\Tx_GoldIcon.dds"));
    assert_eq!(key, "69636f6e732f74785f676f6c6469636f6e2e646473");
    assert_eq!(decode_archive_path_hex(&key)?, b"icons/tx_goldicon.dds");

    let bytes = dream_archivetool::ArchiveTool::read_entry_by_path_bytes(
        "Morrowind.bsa",
        &decode_archive_path_hex(&key)?,
    )?;
    println!("{} bytes", bytes.len());
    Ok(())
}
```

All three are also exported from the crate root.

## ArchiveError

{{ api_signature(value="enum ArchiveError") }}

Every error the crate returns. `Debug`, `Display`, `std::error::Error`, and `From<io::Error>`.
`#[non_exhaustive]`: match it with a `_` arm.

| Variant | Displays as | When |
|---|---|---|
| `UnknownFormat` | `unsupported or unrecognized archive format` | `guess_format` found no BSA or BA2 header |
| `EntryNotFound(String)` | `archive entry not found: <name>` | No entry has that normalized name |
| `UnsafePath(String)` | `unsafe archive path: <name>` | A name extraction or packing refuses |
| `TargetExists(String)` | `target already exists: <path>` | A target exists under `OverwriteMode::Fail` |
| `Io(io::Error)` | `I/O error: <error>` | Reading or writing a file failed |
| `Archive(String)` | `archive error: <message>` | Everything else: an archive that cannot be opened or parsed, a refused rewrite or option, a duplicate name, dream_archive's own errors |

`Archive` carries a message rather than a structured cause; its text is what the command line
prints after `ERROR:`.

{{ api_signature(value="type Result<T> = std::result::Result<T, ArchiveError>") }}

## The dream_archive re-export

{{ api_signature(value="pub use dream_archive") }}

The dream_archive this crate was built against, as `dream_archivetool::dream_archive`. Use it to
go below the policy layer, to its archive types, builders and Luau extension, without a second
dependency that could resolve to another version. Upgrading dream_archivetool can upgrade it.
[dream_archive's documentation](https://DreamWeave-MP.github.io/dream_archive/) covers its API.
