+++
title = "Extracting"
description = "Take one file, or every file, out of an archive: where files land, what happens to files you already have, the names it refuses, and how files are written."
weight = 30

[extra]
kind = "guide"
+++

`extract` takes one file out; `extract-all` takes everything. Both check every name and every
target before writing, and neither replaces a file you already have unless you say so.

## One file

```sh
dream_archivetool extract Morrowind.bsa icons/tx_goldicon.dds --output out
```

The name is matched the way the game matches it: ASCII case does not matter, and `\` is the
same as `/`. So `'Icons\Tx_GoldIcon.dds'`, copied out of a plugin, finds the same file. The file
is written under `--output`, or under the current folder without it, at its
[normalized](@/docs/paths.md) path: `out/icons/tx_goldicon.dds`, in lowercase whatever you typed.

| Option | Does |
|---|---|
| `-o`, `--output <DIR>` | Write under `DIR` instead of the current folder. Created if missing |
| `--flat` | Drop the archive's folders: `out/tx_goldicon.dds` |
| `--overwrite` | Replace the file if it exists |
| `--skip-existing` | Leave it if it exists, and report it as skipped |
| `--fsync` | Flush the file and its folders to disk before finishing |
| `--json` | Print `{ "extracted": 1, "skipped": 0 }` instead of lines |
| `--stdout` | Write the file's bytes to standard output, and nothing else |

`--stdout` writes nothing to disk, so it takes none of the other options:

```sh
dream_archivetool extract Morrowind.bsa icons/tx_goldicon.dds --stdout > gold.dds
```

### Names a shell cannot type

`list --json` gives each entry a `path_bytes_hex`: its name as hexadecimal bytes. Pass that
instead of the name when the name is not valid UTF-8, or when quoting it is more trouble than it
is worth:

```sh
dream_archivetool extract Morrowind.bsa --entry-hex 69636f6e732f74785f676f6c6469636f6e2e646473 --output out
```

The bytes are matched like any name, case and slashes included. On Linux and macOS a name
argument that is not UTF-8 is also taken byte for byte; on Windows it cannot be, and
`--entry-hex` is the way. [Archive paths](@/docs/paths.md#path-and-path-bytes-hex) explains the key.

## Everything

```sh
dream_archivetool extract-all Morrowind.bsa --output Morrowind
```

Every named file in the archive, each at its normalized path under `--output`. `extract-all`
takes `--output`, `--overwrite`, `--skip-existing`, `--fsync` and `--json` as above, and
`--dry-run`. There is no `--flat`: two files with one name in different folders would collide.

Before it writes anything, it builds the whole list of targets and checks it:

- every name must be [safe](@/docs/paths.md#unsafe-names), or it stops at the first that is not;
- no two entries may land on the same file;
- under the default policy, no target may exist yet.

So an `extract-all` that fails has written nothing. An archive with entries that have no name,
such as a hash-only TES4 archive, is refused outright: extracting the rest would silently leave
files out.

```text
ERROR: archive error: archive contains entries without recoverable paths; refusing to extract it lossy
```

## Files you already have

| Policy | Option | A target that exists |
|---|---|---|
| Fail | none, the default | Stops before anything is written: `target already exists: out/icons/tx_goldicon.dds` |
| Overwrite | `--overwrite` | Is replaced |
| Skip | `--skip-existing` | Is left alone and counted as `skipped`; the archive's copy is not even read |

Only one of `--overwrite` and `--skip-existing` can be given.

## Dry runs

`extract-all --dry-run` prints what the extraction would do as JSON, one entry per file, and
writes nothing:

```json
{
  "operation": "extract-all",
  "archive": "MyMod.bsa",
  "output": "out",
  "entries": [
    {
      "action": "extract",
      "path": "meshes/lantern.nif",
      "path_bytes_hex": "6d65736865732f6c616e7465726e2e6e6966",
      "target": "out/meshes/lantern.nif"
    },
    {
      "action": "conflict",
      "path": "textures/tx/tx_lantern.dds",
      "path_bytes_hex": "74657874757265732f74782f74785f6c616e7465726e2e646473",
      "target": "out/textures/tx/tx_lantern.dds"
    },
    {
      "action": "extract",
      "path": "readme.txt",
      "path_bytes_hex": "726561646d652e747874",
      "target": "out/readme.txt"
    }
  ]
}
```

| `action` | The target | And the extraction would |
|---|---|---|
| `extract` | Does not exist | Write it |
| `overwrite` | Exists, with `--overwrite` | Replace it |
| `skip` | Exists, with `--skip-existing` | Leave it |
| `conflict` | Exists, under the default policy | Fail, writing nothing |

A plan never fails because a target exists: that is what `conflict` is for. It fails for the
reasons the extraction itself would fail before writing, such as an unsafe name.

## How a file is written

Each file is written to a temporary file in its target folder, then renamed over the target in
one step, so a crash or a full disk never leaves half a file where a whole one was. Missing
folders are created first. `--fsync` then flushes the file, and on Linux and macOS the folders it
created or changed, so the result survives a power cut; without it, the system writes them back
when it pleases, which is faster.

On Linux and macOS every extracted file is readable and writable by you alone (mode `0600`),
whatever your umask: it starts life as a private temporary file and keeps that mode. Run
`chmod -R a+r` on the output if other users or a server need to read it.

## What the checks do not do

The name checks stop an archive from naming a file outside `--output`. They do not make the
output folder a jail: if a folder inside it is a symbolic link you made, files extracted there
follow it. Extract into a folder whose existing contents you trust.

From Rust and Luau, `extract_many` and `extractMany` take a list of entries in one pass, with the
same checks and the same all-or-nothing start. See the [Rust API](@/docs/api/extracting.md) and
the [archive methods](@/docs/luau/archive.md).
