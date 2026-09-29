+++
title = "Inspecting archives"
description = "info, list, verify and diff: what is in an archive, whether it is healthy, and what changed between two of them."
weight = 20

[extra]
kind = "guide"
+++

Four commands read an archive and write nothing. Each prints a few lines for people, or with
`--json` the whole report for scripts; [JSON output](@/docs/json.md) has every field.

## info

```sh
dream_archivetool info Morrowind.bsa
```

```text
format: tes3
files: 11090
```

`format` is `tes3` for a Morrowind BSA, `tes4` for Oblivion's through Skyrim Special Edition's,
and `ba2` for Fallout 4's and Starfield's. `--json` adds whether the archive can be
[rewritten](@/docs/creating.md#what-it-refuses-to-rewrite), and the header facts that decide
it:

```sh
dream_archivetool info --json MyMod4.bsa
```

```json
{
  "path": "MyMod4.bsa",
  "format": "tes4",
  "file_count": 3,
  "named_entry_count": 3,
  "has_unnameable_entries": false,
  "rewritable": true,
  "tes4": {
    "version": "v105",
    "archive_types": "ArchiveTypes(256)",
    "archive_types_bits": 256,
    "archive_flags": [
      "directory-strings",
      "file-strings",
      "compressed"
    ],
    "archive_flags_bits": 7,
    "unsupported_archive_flags_bits": 0,
    "name_mode": "strings"
  }
}
```

A TES4 archive's `name_mode` says where its file names are: `strings` (the name tables),
`embedded` (beside each file), `strings-and-embedded`, or `hash-only`, where there are no names
at all, only the hashes the game looks files up by. A BA2 gets a `ba2` object instead, with its
`version`, `payload_format` (`gnrl` for general files, `dx10` for textures) and whether it has a
name table (`strings`).

## list

```sh
dream_archivetool list Morrowind.bsa
dream_archivetool list --long Morrowind.bsa
dream_archivetool list --json Morrowind.bsa
```

One path per line, in the archive's own order. Paths are shown
[normalized](@/docs/paths.md): lowercase ASCII, forward slashes. `--long` puts the size in front,
right-aligned in ten columns; a TES4 archive does not record sizes in its index, so its sizes
are `-`. `--json` gives each entry's path, its `path_bytes_hex` lookup key, its size and, for a
compressed entry, its stored size.

An entry with no name, as in a hash-only TES4 archive, is not listed: there is nothing to print
and nothing to extract it by. `info` counts it, and `verify` reports it.

## verify

```sh
dream_archivetool verify Morrowind.bsa --read-payloads
```

```text
format: tes3
files: 11090
named: 11090
unnameable: 0
rewritable: true
payloads read: 11090
```

| Line | Means |
|---|---|
| `named`, `unnameable` | Entries with a recoverable file name, and entries with only a hash |
| `rewritable` | Whether `add` can write this archive again without losing anything |
| `rewrite blocker: ...` | Why not, when `rewritable` is false |
| `payloads read: N` | With `--read-payloads`: every named file was decompressed and read to the end, and none failed |
| `warning: ...` | Something to know, one line each |

`verify` without `--read-payloads` reads only the index, which is fast; with it, it reads every
file, which is how you find a corrupt one. A file that cannot be read stops `verify` with an
error and exit code 1. Everything else is a report, and the exit code is 0.

The warnings, and what `--json` lists for each:

| Warning | In `--json` |
|---|---|
| `archive contains entries without recoverable path names` | `unnameable_entries` |
| `archive contains duplicate normalized paths` | `duplicate_normalized_paths`: two names that differ only in case or slashes, which the game cannot tell apart |
| `payload read verification skipped because duplicate normalized paths prevent per-entry coverage` | `payloads_read` is `null`: a lookup by name would read the same file twice |
| `archive contains paths unsafe to extract directly` | `unsafe_paths`: names that would land outside the output folder, or that your system cannot hold |

A damaged archive, with two spellings of one texture and a name that climbs out of its folder:

```text
format: tes3
files: 4
named: 4
unnameable: 0
rewritable: true
warning: archive contains duplicate normalized paths
warning: archive contains paths unsafe to extract directly
```

`extract-all`, `diff` and `add` refuse that archive, each naming the problem. `extract` still
takes single files out of it by name, except the unsafe one.

## diff

```sh
dream_archivetool diff MyMod.bsa MyMod-2.bsa
```

```text
comparison: metadata-only
added: 1
removed: 0
changed: 1
unchanged: 2
```

Entries are matched by normalized path, so `Textures\A.dds` in one archive and `textures/a.dds`
in the other are the same file. Without `--hash`, two entries are the same when their size and
stored size are; that is quick, and it misses a file whose bytes changed but whose size did not.
A TES4 archive's index has no sizes at all, only the stored size of a compressed file, so
between uncompressed TES4 archives every entry present in both counts as unchanged. Use `--hash`
there.

`--hash` reads both archives in full and compares a fingerprint of every file's bytes: 64-bit
FNV-1a, fast and good at telling files apart, and no defense against someone who makes two files
collide on purpose. `--json` lists the added, removed and changed entries, each with its
fingerprint:

```sh
dream_archivetool diff MyMod.bsa MyMod-2.bsa --hash --json
```

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

The two archives can be different formats. `diff` refuses an archive with unnamed entries or
with duplicate normalized paths, because either would make the comparison guess.
