+++
title = "Archive paths"
description = "How entry names are normalized and matched, the difference between path and path_bytes_hex, the names extraction refuses, and how files on disk become archive names."
weight = 70

[extra]
kind = "reference"
+++

An archive names its files with bytes, spelled however the tool that made it spelled them.
dream_archivetool never compares those bytes directly: it compares their normalized form, the
same key the game and every DreamWeave tool look files up by.

## Normalization

The rules are [dream-path](https://DreamWeave-MP.github.io/dream_path/)'s, through
dream_archive:

| Input | Becomes |
|---|---|
| `\` | `/` |
| ASCII `A` to `Z` | `a` to `z` |
| Two or more separators in a row | One `/` |
| Separators at the start | Nothing |

Every other byte is kept, non-ASCII letters and invalid UTF-8 included. `..` is not resolved;
it is two dots. [dream-path's rules](https://DreamWeave-MP.github.io/dream_path/docs/rules/) have
the full table.

Everything goes through it:

- **Names in an archive** are listed, compared and extracted by their normalized form, so
  `list` prints `textures/tx_wood.dds` for an entry stored as `Textures\Tx_Wood.DDS`.
- **Names you ask for** are normalized before the lookup, so any spelling of a name finds it.
- **Extracted files** are written at the normalized path: lowercase, whatever the archive or you
  typed.
- **Files you pack** are stored under their normalized names.

## path and path_bytes_hex

Every entry in JSON output, and every row in a report or plan, carries two spellings of its
name:

| Field | Is | For |
|---|---|---|
| `path` | The normalized name as text. Bytes that are not UTF-8 become `�` | Reading |
| `path_bytes_hex` | The normalized name's bytes, as lowercase hexadecimal | Scripts: `extract --entry-hex`, and the Luau `ByPathHex` functions |

```json
{
  "path": "icons/tx_goldicon.dds",
  "path_bytes_hex": "69636f6e732f74785f676f6c6469636f6e2e646473",
  "size": 448,
  "compressed_size": null
}
```

`path_bytes_hex` survives every name, including ones that are not text, which is why scripts
should keep it rather than `path`. It is a lookup key, not an identity: two entries stored as
`Textures\A.dds` and `textures/a.dds` have the same key, and a lookup by that key finds the
first. `verify` reports such pairs, with each one's stored bytes in `raw_path_bytes_hex` and
`colliding_raw_path_bytes_hex`.

## Unsafe names

A name is refused as a target for extraction, from the archive or from you, when it:

- is empty, or is nothing but separators and `.`;
- starts with `/` or `\`, as an absolute path does;
- contains a NUL byte;
- has a `..` component;
- has a component containing `:`, as `C:` or a Windows alternate stream does.

On Windows and macOS, each component must also be valid UTF-8, and one ordinary name on that
system. On Linux, a component can be any bytes, and the file is created with exactly those bytes
as its name.

The error is `unsafe archive path: ...`, before anything is written. `verify` lists every
unsafe name in an archive without extracting anything, and `extract --stdout`, which writes no
file, can still read an entry whose name is unsafe to create on disk.

These rules keep an archive's names inside the output folder. They do not stop a symbolic link
already in the output folder from leading elsewhere: see
[Extracting](@/docs/extracting.md#what-the-checks-do-not-do).

## Files on disk

`create` and `add` turn each input file's path, relative to the folder you gave, into a name:
its components joined with `/`, then normalized, then checked as above. A folder's own name is
not part of it, and a single file is stored under its file name alone:

| Command | File | Stored as |
|---|---|---|
| `create MyMod.bsa MyMod ...` | `MyMod/Textures/Tx_Lantern.dds` | `textures/tx_lantern.dds` |
| `create MyMod.bsa MyMod/readme.txt ...` | `MyMod/readme.txt` | `readme.txt` |
| `add MyMod.bsa Update ...` | `Update/meshes/lantern.nif` | `meshes/lantern.nif` |

The stored bytes are the file name as your system gives it: UTF-8 on Windows and macOS, and
whatever the name is on Linux. The games read names in their own legacy code page, so a name
that is plain ASCII is the one that works everywhere.
