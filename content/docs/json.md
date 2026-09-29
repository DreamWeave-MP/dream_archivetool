+++
title = "JSON output"
description = "Every shape --json and --dry-run print: info, list, verify, diff, the extraction summary, the file count, and the three plans."
weight = 60

[extra]
kind = "reference"
+++

With `--json`, and always with `--dry-run`, a command prints one JSON value to standard output,
indented two spaces and followed by a newline, and nothing else. Errors still go to standard
error as `ERROR: ...`, with exit code 1. The Rust types behind each shape are in the
[Rust API](@/docs/api/_index.md); they serialize to exactly this.

Field names are `snake_case` and string values `kebab-case`. The Luau API spells the same facts in
camelCase: `path_bytes_hex` is `pathBytesHex`, `"tes4"` is `"bsaTes4"`.

**Compatibility.** A minor release may add fields; removing or renaming one is a breaking
release. Read the fields you need and ignore the rest.

## Values used throughout

| Field | Values |
|---|---|
| `format` | `"tes3"`, `"tes4"` or `"ba2"` |
| `path` | The normalized name as text, for people. See [Archive paths](@/docs/paths.md#path-and-path-bytes-hex) |
| `path_bytes_hex` | The normalized name's bytes in lowercase hex, for scripts |
| `size` | The file's size in bytes; `null` only for a TES4 file whose data lies outside the archive |
| `compressed_size` | The stored size of a compressed file, or `null` when it is stored uncompressed |

## info

```json
{
  "path": "hashonly.bsa",
  "format": "tes4",
  "file_count": 1,
  "named_entry_count": 0,
  "has_unnameable_entries": true,
  "rewritable": false,
  "rewrite_blocker": "archive contains entries without recoverable paths; refusing to rewrite it lossy",
  "tes4": {
    "version": "v104",
    "archive_types": "ArchiveTypes(256)",
    "archive_types_bits": 256,
    "archive_flags": [],
    "archive_flags_bits": 0,
    "unsupported_archive_flags_bits": 0,
    "name_mode": "hash-only"
  }
}
```

| Field | Is |
|---|---|
| `path` | The archive's path as given |
| `file_count` | Every entry, named or not |
| `named_entry_count` | Entries with a file name |
| `has_unnameable_entries` | Whether any entry has only a hash |
| `rewritable` | Whether `add` would rewrite it |
| `rewrite_blocker` | Why not. Absent when `rewritable` is true |
| `tes4` | TES4 archives only. Absent otherwise |
| `ba2` | BA2 archives only. Absent otherwise |

`tes4`:

| Field | Is |
|---|---|
| `version` | `"v103"` (Oblivion), `"v104"` (Fallout 3, Skyrim) or `"v105"` (Skyrim Special Edition) |
| `archive_types`, `archive_types_bits` | The header's content-type bits, as dream_archive prints them and as a number. Read the number |
| `archive_flags` | The flags this tool knows, by name: `"directory-strings"`, `"file-strings"`, `"compressed"`, `"embedded-file-names"` |
| `archive_flags_bits` | Every flag bit, as a number |
| `unsupported_archive_flags_bits` | The bits outside those four. Anything but 0 blocks a rewrite |
| `name_mode` | Where the names are: `"strings"`, `"embedded"`, `"strings-and-embedded"` or `"hash-only"` |

`ba2`:

```json
{
  "version": "v2",
  "payload_format": "gnrl",
  "compression_format": "zip",
  "strings": true
}
```

| Field | Is |
|---|---|
| `version` | `"v1"`, `"v2"`, `"v3"`, `"v7"` or `"v8"` |
| `payload_format` | `"gnrl"` (general files), `"dx10"` (textures) or `"gnmf"` (console textures) |
| `compression_format` | `"zip"` or `"lz4"`: the method compressed files use. It does not say that any are compressed |
| `strings` | Whether the archive has a name table |

## list

An array, one object per named entry, in the archive's order:

```json
[
  {
    "path": "readme.txt",
    "path_bytes_hex": "726561646d652e747874",
    "size": 5,
    "compressed_size": 13
  }
]
```

## verify

```json
{
  "path": "damaged.bsa",
  "format": "tes3",
  "file_count": 4,
  "named_entry_count": 4,
  "unnameable_entries": 0,
  "rewritable": true,
  "duplicate_normalized_paths": [
    {
      "path": "textures/a.dds",
      "path_bytes_hex": "74657874757265732f612e646473",
      "raw_path_bytes_hex": "54657874757265735c412e444453",
      "colliding_raw_path_bytes_hex": "74657874757265735c612e646473"
    }
  ],
  "unsafe_paths": [
    {
      "path": "../../../c.dds",
      "path_bytes_hex": "2e2e2f2e2e2f2e2e2f632e646473",
      "raw_path_bytes_hex": "2e2e5c2e2e5c2e2e5c632e646473"
    }
  ],
  "payloads_read": null,
  "warnings": [
    "archive contains duplicate normalized paths",
    "payload read verification skipped because duplicate normalized paths prevent per-entry coverage",
    "archive contains paths unsafe to extract directly"
  ]
}
```

| Field | Is |
|---|---|
| `path`, `format`, `file_count`, `named_entry_count`, `rewritable`, `rewrite_blocker` | As in `info` |
| `unnameable_entries` | How many entries have only a hash |
| `duplicate_normalized_paths` | One object for each entry whose normalized name an earlier entry already has: the shared key, this entry's stored bytes in `raw_path_bytes_hex`, and the earlier one's in `colliding_raw_path_bytes_hex` |
| `unsafe_paths` | One object for each name extraction would refuse: its normalized key, and its stored bytes in `raw_path_bytes_hex` |
| `payloads_read` | With `--read-payloads`, how many files were read to the end. `null` without it, or when duplicates made reading by name ambiguous |
| `warnings` | The lines `verify` prints as `warning:` |

`path` and `path_bytes_hex` are the normalized key here as everywhere else: above, the stored
`..\..\..\c.dds` is listed as `../../../c.dds`, as `list` shows it.

## diff

```json
{
  "old": "MyMod.bsa",
  "new": "MyMod-2.bsa",
  "comparison": "payload-fingerprint",
  "fingerprint_payloads": true,
  "added": [
    {
      "path": "icon.txt",
      "path_bytes_hex": "69636f6e2e747874",
      "size": 4,
      "compressed_size": null,
      "payload_fingerprint": "dbff4cc56c2092f0"
    }
  ],
  "removed": [],
  "changed": [
    {
      "path": "textures/tx/tx_lantern.dds",
      "path_bytes_hex": "74657874757265732f74782f74785f6c616e7465726e2e646473",
      "old": {
        "size": 24,
        "compressed_size": null,
        "payload_fingerprint": "94f2d9697dfa340a"
      },
      "new": {
        "size": 25,
        "compressed_size": null,
        "payload_fingerprint": "12edd4b0595668a3"
      }
    }
  ],
  "unchanged": 2
}
```

| Field | Is |
|---|---|
| `old`, `new` | The two paths as given |
| `comparison` | `"metadata-only"`, or `"payload-fingerprint"` with `--hash` |
| `fingerprint_payloads` | Whether `--hash` was given |
| `added` | Entries only in `new`, in name order |
| `removed` | Entries only in `old`, in name order |
| `changed` | Entries in both whose size, stored size or fingerprint differ, with each side's values in `old` and `new` |
| `unchanged` | How many entries are in both and the same |

`payload_fingerprint` is the 64-bit FNV-1a hash of the file's bytes as 16 hex digits. Without
`--hash` it is absent, not `null`.

## Extraction summary

`extract --json` and `extract-all --json`:

```json
{
  "extracted": 3,
  "skipped": 0
}
```

`skipped` counts targets left alone under `--skip-existing`.

## File count

`create --json` and `add --json`:

```json
{
  "files": 4
}
```

Every file in the archive written, including, for `add`, the ones kept from the old archive.

## Extraction plan

`extract-all --dry-run`:

| Field | Is |
|---|---|
| `operation` | `"extract-all"`. A plan for chosen entries, from the library, says `"extract"` |
| `archive` | The archive's path as given |
| `output` | The output folder: `--output`, or `"."` |
| `entries` | One object per file: `action`, `path`, `path_bytes_hex`, and `target`, the file it would write |

`action` is `"extract"`, `"overwrite"`, `"skip"` or `"conflict"`; [Extracting](@/docs/extracting.md#dry-runs)
says when each applies. [Start here](@/docs/start-here.md#take-everything-out) shows one.

## Creation plan

`create --dry-run`:

```json
{
  "operation": "create",
  "format": "tes3",
  "output": "MyMod.bsa",
  "files": 1,
  "entries": [
    {
      "action": "add",
      "source": "MyMod/readme.txt",
      "path": "readme.txt",
      "path_bytes_hex": "726561646d652e747874",
      "size": 5
    }
  ]
}
```

`files` is how many files the archive would hold; each entry has the file it comes from in
`source` and that file's `size`, in name order.

## Update plan

`add --dry-run`, as in [Creating and updating](@/docs/creating.md#dry-runs):

| Field | Is |
|---|---|
| `operation` | `"add"` |
| `archive`, `output` | The archive, and where the new one would go: `--output`, or the archive itself |
| `format` | The archive's format, which the new one keeps |
| `files` | How many files the new archive would hold |
| `added`, `replaced`, `preserved` | How many are new, replace an entry, or are kept from the old archive |
| `entries` | One object per file. `action` is `"add"`, `"replace"` or `"preserve"`; kept entries have no `source` or `size` |
