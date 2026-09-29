+++
title = "dream_archivetool"
description = "Inspect, extract, create and update Bethesda BSA and BA2 archives: a command line, a Rust library, and a Luau extension."

[taxonomies]
tags = ["Morrowind", "Oblivion", "Skyrim", "Fallout 4", "Starfield", "BSA", "BA2", "Rust", "Luau"]
+++

Bethesda's games keep their meshes, textures and sounds in archives: `.bsa` from Morrowind to
Skyrim, `.ba2` from Fallout 4 on. dream_archivetool opens all of them, lists and checks what is
inside, extracts it, and packs folders into new ones.

```sh
dream_archivetool info Morrowind.bsa
dream_archivetool extract-all Morrowind.bsa --output Morrowind
dream_archivetool create MyMod.bsa MyMod --format tes3
```

It is careful about the two things archive tools get wrong. The paths inside an archive were
written by whoever made it, so every target is checked before a byte is written: nothing lands
outside the folder you named, and nothing you already have is replaced unless you say so. And an
archive it cannot rewrite without losing something, such as names stored only as hashes, it
refuses to rewrite, before the file is touched. Every command that writes can print its plan
first instead.

{{ schematic(data_path="data/schematics/policy.json") }}

- **Every Bethesda archive**: Morrowind's BSAs; Oblivion, Fallout 3, Skyrim and Skyrim Special
  Edition BSAs; Fallout 4, Fallout 4 next-gen and Starfield BA2s, general and texture.
- **Scriptable**: `--json` on every command, one stable path key per entry, and nothing but the
  file on standard output with `--stdout`.
- **Everywhere**: Windows, Linux and macOS, Android in a terminal, and PortMaster handhelds.
- **A library**: the same operations as a Rust crate, and as a Luau extension that gives
  [dream_archive](https://DreamWeave-MP.github.io/dream_archive/)'s archives the methods to
  verify, compare and extract themselves.

The formats themselves are dream_archive's business: dream_archivetool never parses an archive,
it decides what is safe to do with one.

## Documentation

- **[Start here](@/docs/start-here.md)**: download it, look inside an archive, take files out and
  pack a folder in.
- **[Extracting](@/docs/extracting.md)** and **[Creating and updating](@/docs/creating.md)**:
  every choice those commands make, and why they refuse what they refuse.
- **[Command line](@/docs/cli.md)** and **[JSON output](@/docs/json.md)**: every option, the exit
  codes, and the shapes scripts read.
- **[Rust API](@/docs/api/_index.md)** and **[Luau API](@/docs/luau/_index.md)**: the tool as a
  library.
