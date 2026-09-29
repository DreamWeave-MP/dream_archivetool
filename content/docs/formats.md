+++
title = "Formats"
description = "The BSA and BA2 variants dream_archivetool reads and writes, what each records about its files, what create writes in the header, and what an update keeps."
weight = 75

[extra]
kind = "reference"
+++

Reading and writing the formats is [dream_archive](https://DreamWeave-MP.github.io/dream_archive/)'s
work; its documentation describes the bytes. This page is what those formats mean for
dream_archivetool's commands.

## What it reads

| `format` | Archives | Games |
|---|---|---|
| `tes3` | BSA | Morrowind |
| `tes4` | BSA versions 103, 104 and 105 | Oblivion; Fallout 3, New Vegas and Skyrim; Skyrim Special Edition |
| `ba2` | BA2 versions 1, 2, 3, 7 and 8, general (`GNRL`) and texture (`DX10`) | Fallout 4 and its next-gen update, Fallout 76, Starfield |

The format is found from the file's header, never from its extension. A BA2 of console textures
(`GNMF`) can be opened and inspected, but dream_archive does not extract or write its textures.

## What each records

| | TES3 | TES4 | BA2 |
|---|---|---|---|
| Names | Always | In name tables, beside each file, both, or not at all (hash-only) | In a name table, when the archive has one |
| `size` | Yes | Yes: an uncompressed file's from the index, a compressed file's from the first four bytes of its data | Yes |
| `compressed_size` | Never compressed | For compressed files | For compressed files |

An entry without a name can be counted but not listed, extracted by name, compared or rewritten.
The commands that would need its name refuse the archive rather than leave it out.

## What create writes

| Options | Header |
|---|---|
| `--format tes3` | A Morrowind BSA |
| `--format tes4` | Version 103; `--tes4-version fallout3` and `skyrim` write 104, `skyrim-se` 105 |
| `--format ba2` | Version 1; `--ba2-version starfield` writes 2, `fallout4-next-gen` 8 |

Every TES4 archive `create` writes has both name tables and the miscellaneous content type (256
in `archive_types_bits`). Every BA2 has a name table, so it can be listed and extracted by name.

`create` hands dream_archive each input as a file path, to be read while the archive is written,
rather than loading every file first.

`--compress` sets the TES4 archive's compressed flag, which compresses every file with zlib, or
with LZ4 in version 105. In a BA2 it compresses every file with zlib. A `dx10` BA2 stores each
texture as the chunks the game streams, which dream_archive builds from the DDS header, so each
input must be a valid DDS file.

## What an update keeps

`add` writes a new archive of the same format, and carries over what dream_archive lets it
read back:

| Format | Kept | Not kept |
|---|---|---|
| TES3 | Everything; the format has no settings | |
| TES4 | Version, content types, the compressed flag (new files are compressed when the archive is), where names are stored | |
| BA2, general | Version, name table, compression: each kept file stays compressed or stored, and new files are compressed with the archive's method when any file in it is | |
| BA2, textures | Version, name table; kept textures are copied as their stored chunks, compression and all; new textures are compressed when any texture is, or when the archive's method is LZ4 | |

The files kept from the old archive are not loaded into memory first: each is read from the old
archive while the new one is written. An update refuses what it cannot carry over;
[Creating and updating](@/docs/creating.md#what-it-refuses-to-rewrite) lists each case.
