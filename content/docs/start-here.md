+++
title = "Start here"
description = "Download dream_archivetool, look inside an archive, take files out, and pack a folder into a new archive."
weight = 10

[extra]
kind = "tutorial"
+++

You need an archive to look at. Any game's will do; this page uses Morrowind's own
`Morrowind.bsa`, from its `Data Files` folder.

## Download it

Take the archive for your system from the [project page](@/home/index.md) and unzip it
anywhere. It holds one program, `dream_archivetool`, with its README and license. There is
nothing to install, and no window: it runs in a terminal.

- **Windows**: open PowerShell in the folder and run `.\dream_archivetool.exe`.
- **macOS**: the builds are not notarized. If macOS will not run it, run
  `xattr -d com.apple.quarantine dream_archivetool` in its folder, or allow it under System
  Settings, Privacy & Security.
- **Linux and macOS**: if the shell says permission denied, `chmod +x dream_archivetool`.
- **Android and handhelds**: the same program, for a terminal. See
  [Platforms](@/docs/compatibility.md#downloads).

`dream_archivetool --help` lists the commands. The examples below assume the program is on your
`PATH`, or that you type its full path.

## Look inside

```sh
dream_archivetool info Morrowind.bsa
```

```text
format: tes3
files: 11090
```

`list` prints every path in the archive, one per line, and `--long` puts each file's size in
front:

```sh
dream_archivetool list --long Morrowind.bsa
```

```text
      6276 meshes/m/probe_journeyman_01.nif
       256 textures/menu_rightbuttonup_top.dds
       256 textures/menu_rightbuttonup_right.dds
       256 textures/menu_rightbuttonup_left.dds
```

That goes on for 11090 lines. `verify` checks that every name is safe to extract and, with
`--read-payloads`, that every file in the archive can actually be read:

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

[Inspecting archives](@/docs/inspecting.md) explains each line.

## Take one file out

Name the file as the archive does. Case and slashes do not matter, so `Icons\Tx_GoldIcon.dds`
from a plugin works as well as `icons/tx_goldicon.dds`:

```sh
dream_archivetool extract Morrowind.bsa 'Icons\Tx_GoldIcon.dds' --output out
```

```text
extracted: 1
```

The file lands at `out/icons/tx_goldicon.dds`, its folders kept. Without `--output`, it goes
under the current folder. Run the same command again and it stops instead of replacing the file:

```text
ERROR: target already exists: out/icons/tx_goldicon.dds
```

`--overwrite` replaces it; `--skip-existing` leaves it and says so.

## Take everything out

Before writing eleven thousand files, ask what would happen:

```sh
dream_archivetool extract-all Morrowind.bsa --output out --dry-run
```

The answer is JSON, one entry per file, and nothing is written:

```json
{
  "operation": "extract-all",
  "archive": "Morrowind.bsa",
  "output": "out",
  "entries": [
    {
      "action": "extract",
      "path": "meshes/m/probe_journeyman_01.nif",
      "path_bytes_hex": "6d65736865732f6d2f70726f62655f6a6f75726e65796d616e5f30312e6e6966",
      "target": "out/meshes/m/probe_journeyman_01.nif"
    }
  ]
}
```

Here 11089 entries say `extract`, and one says `conflict`: the icon you already took out. Run it
without `--dry-run` and it refuses the whole extraction, writing nothing, because of that one
file. Add `--skip-existing` to keep your copy and extract the rest:

```sh
dream_archivetool extract-all Morrowind.bsa --output out --skip-existing
```

```text
extracted: 11089
skipped: 1
```

## Pack a folder

A mod's files, laid out as the game expects them:

{% tree() %}
MyMod/
  meshes/
    lantern.nif
  textures/
    tx/
      tx_lantern.dds
  readme.txt
{% end %}

```sh
dream_archivetool create MyMod.bsa MyMod --format tes3
```

```text
files: 3
```

The folder itself is not part of the names: the archive holds `meshes/lantern.nif`, not
`MyMod/meshes/lantern.nif`. `--format tes3` is Morrowind's BSA; `tes4` with a `--tes4-version`
is Oblivion's to Skyrim Special Edition's, and `ba2` is Fallout 4's and Starfield's.
[Creating and updating](@/docs/creating.md) covers each.

To add files later, or replace some, put them in a folder laid out the same way and `add` it. The
archive is written again with them in:

```sh
dream_archivetool add MyMod.bsa MyModUpdate
```

Every command that writes can print its plan with `--dry-run` first.

## If it stops

Errors start with `ERROR:` and the exit code is 1. Nothing was written unless the message says
otherwise.

- **`failed to open archive 'x.bsa': No such file or directory (os error 2)`**: the path is
  wrong.
- **`failed to open archive 'x.bsa': unknown or disabled archive format`**: the file is not a
  BSA or BA2.
- **`archive entry not found: ...`**: no file in the archive has that name, even ignoring case.
  `list` shows the names it has.
- **`target already exists: ...`**: see above; choose `--overwrite` or `--skip-existing`.
- **`unsafe archive path: ...`**: the archive names a file outside the output folder, or with a
  name your system cannot hold. See [Archive paths](@/docs/paths.md#unsafe-names).
