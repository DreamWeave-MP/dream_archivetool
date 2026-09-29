+++
title = "Creating and updating"
description = "Pack a folder into a BSA or BA2, choose the game's format, add and replace files in an archive, and what dream_archivetool refuses to rewrite."
weight = 40

[extra]
kind = "guide"
+++

`create` packs a folder into a new archive. `add` writes an existing archive again with files
added or replaced. Both check every name before writing, write through a temporary file, and can
print their plan with `--dry-run` instead.

## create

```sh
dream_archivetool create MyMod.bsa MyMod --format tes3
```

The second argument is a folder or a single file. A folder's contents are stored relative to it,
so `MyMod/meshes/lantern.nif` becomes `meshes/lantern.nif` and the folder's own name is not in
the archive. A single file is stored under its file name, at the archive's root. Each name is
[normalized](@/docs/paths.md) on the way in: lowercase ASCII, forward slashes. Two files whose
names differ only in case are an error, since the game could only ever load one of them:

```text
ERROR: archive error: duplicate archive path after normalization: textures/a.dds (first source: Dup/Textures/a.dds, duplicate source: Dup/textures/A.DDS)
```

The new archive is written to a temporary file next to `ARCHIVE` and renamed into place when it
is complete. A file already at `ARCHIVE` is replaced then, and not before: a failed `create`
leaves it as it was. The program prints how many files it wrote, `files: 3`, or with `--json`,
`{ "files": 3 }`.

## Choosing the format

`--format` is required.

| Game | Options |
|---|---|
| Morrowind | `--format tes3` |
| Oblivion | `--format tes4`, or `--format tes4 --tes4-version oblivion` |
| Fallout 3, Fallout: New Vegas | `--format tes4 --tes4-version fallout3` |
| Skyrim | `--format tes4 --tes4-version skyrim` |
| Skyrim Special Edition | `--format tes4 --tes4-version skyrim-se` |
| Fallout 4 | `--format ba2`, or `--format ba2 --ba2-version fallout4` |
| Fallout 4 next-gen update | `--format ba2 --ba2-version fallout4-next-gen` |
| Starfield | `--format ba2 --ba2-version starfield` |

A BA2 holds either general files or textures:

| `--ba2-kind` | Holds |
|---|---|
| `gnrl`, the default | Anything |
| `dx10` | Only `.dds` textures, stored as the game's texture chunks. Each must be a valid DDS file |
| `gnmf` | Console textures. Accepted by the parser and refused before writing: dream_archive does not write them |

`--compress` compresses every file: with zlib in Oblivion, Fallout 3 and Skyrim BSAs and in
BA2s, and with LZ4 in Skyrim Special Edition BSAs. Morrowind's BSA has no compression, so
`--compress` with `--format tes3` is an error, and so is any option that belongs to another
format:

```text
ERROR: archive error: --ba2-kind is not valid with --format tes3
```

[Formats](@/docs/formats.md) has what each choice writes in the header.

## add

```sh
dream_archivetool add MyMod.bsa MyModUpdate
dream_archivetool add MyMod.bsa new_icon.dds --output MyMod-2.bsa
```

Any number of files and folders, each laid out as for `create`: a folder's contents relative to
it, a file under its own name. An input whose normalized name is already in the archive replaces
that entry; any other is added; every entry nothing replaces is kept, copied from the old
archive. Two inputs with the same name are an error.

`add` never edits an archive in place. It writes a complete new archive to a temporary file,
then:

- without `--output`, renames it over the original, so the original changes in one step or not
  at all;
- with `--output FILE`, renames it to `FILE` and leaves the original alone. `FILE` may not be the
  original itself: leave `--output` out for that.

The new archive has the old one's format, version and settings, as far as dream_archive exposes
them; [Formats](@/docs/formats.md#what-an-update-keeps) lists what is kept. The number printed
is every file in the new archive, kept ones included.

## What it refuses to rewrite

Before it reads a single input, `add` asks whether it can write this archive again without
losing anything. When it cannot, it stops and says why, and the archive is untouched:

| Refusal | Why |
|---|---|
| `archive contains entries without recoverable paths; refusing to rewrite it lossy` | Some entries have no name to write them back under |
| `TES4 hash-only archives do not have recoverable path names; refusing to rewrite them lossy` | The same, for a TES4 archive that stores only hashes |
| `TES4 archive uses header flag bits this tool cannot preserve; refusing to rewrite it lossy` | The header sets a flag beyond the name tables, compression and embedded names, and the new archive would drop it |
| `creating or updating GNMF BA2 archives requires console texture swizzle semantics and is not supported by dream_archive` | Console texture archives cannot be written |
| `archive contains duplicate normalized path: ...` | Two entries share a name; writing both back would keep one |

`info --json` and `verify` report the first four as `rewritable` and `rewrite_blocker`, so you
can ask before you try; duplicate names are in `verify`'s `duplicate_normalized_paths`. An update
of a DX10 BA2 takes only `.dds` files, as `create` does.

## Dry runs

`--dry-run` prints the plan as JSON and writes nothing. For `create`, one `add` row per file,
with the file it comes from and its size. For `add`, a row for every entry the new archive
would hold, kept entries first, then the inputs, each in name order:

```sh
dream_archivetool add MyMod.bsa Update --output MyMod-2.bsa --dry-run
```

```json
{
  "operation": "add",
  "archive": "MyMod.bsa",
  "output": "MyMod-2.bsa",
  "format": "tes3",
  "files": 4,
  "added": 1,
  "replaced": 1,
  "preserved": 2,
  "entries": [
    {
      "action": "preserve",
      "path": "meshes/lantern.nif",
      "path_bytes_hex": "6d65736865732f6c616e7465726e2e6e6966"
    },
    {
      "action": "preserve",
      "path": "readme.txt",
      "path_bytes_hex": "726561646d652e747874"
    },
    {
      "action": "add",
      "source": "Update/icon.txt",
      "path": "icon.txt",
      "path_bytes_hex": "69636f6e2e747874",
      "size": 4
    },
    {
      "action": "replace",
      "source": "Update/textures/tx/TX_Lantern.dds",
      "path": "textures/tx/tx_lantern.dds",
      "path_bytes_hex": "74657874757265732f74782f74785f6c616e7465726e2e646473",
      "size": 25
    }
  ]
}
```

The rows are a report, not the order the files will have inside the archive. A plan runs the
same checks on names, inputs and the archive as the real command, and refuses what it would
refuse. What it does not read is the files themselves: a `.dds` that is not a valid texture passes
the plan of a DX10 BA2 and fails the real `create`. `--json` is accepted beside `--dry-run`, and
changes nothing: a plan is always JSON.

## Symbolic links

A symbolic link among the inputs, or an input that is one, is refused by default:

```text
ERROR: archive error: refusing to follow symlink input path: Links/link.txt; pass follow_symlinks to opt in
```

`--follow-symlinks` packs what each link points to, under the link's name. Use it only on a tree
you trust, and that will not change while the archive is being written.

## Durability

`--fsync` flushes the new archive to disk before it is renamed into place, and on Linux and
macOS flushes its folder afterwards, so the rename survives a power cut. Without it, the rename
still happens in one step, and the system writes the data back when it pleases.

On Linux and macOS the new archive is readable and writable by you alone (mode `0600`), because
it starts as a private temporary file. That includes an archive `add` replaces in place: one that
others could read before is yours alone after. `chmod` it back if that matters.
