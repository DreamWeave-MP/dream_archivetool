+++
title = "Command line"
description = "Every dream_archivetool command and option, the rules between them, where output goes, and the exit codes."
weight = 50

[extra]
kind = "reference"
+++

```text
dream_archivetool info [--json] <ARCHIVE>
dream_archivetool list [--long | --json] <ARCHIVE>
dream_archivetool verify [--read-payloads] [--json] <ARCHIVE>
dream_archivetool diff [--hash] [--json] <OLD> <NEW>
dream_archivetool extract [options] <ARCHIVE> (<ENTRY> | --entry-hex <HEX>)
dream_archivetool extract-all [options] <ARCHIVE>
dream_archivetool create --format <FORMAT> [options] <ARCHIVE> <INPUT>
dream_archivetool add [options] <ARCHIVE> <INPUTS>...
dream_archivetool --generate-completion <SHELL>
dream_archivetool --generate-manpage
```

With no command, it prints its help and exits 0. `dream_archivetool help <command>` and
`<command> --help` describe one command; `--version` prints `dream_archivetool 1.0.0`.

## info

Prints the archive's format and file count.

| Argument or option | Meaning |
|---|---|
| `<ARCHIVE>` | The archive |
| `--json` | Print the [whole report](@/docs/json.md#info): names, rewritability and header facts |

## list

Prints every named entry's normalized path, one per line, in the archive's order.

| Argument or option | Meaning |
|---|---|
| `<ARCHIVE>` | The archive |
| `-l`, `--long` | Put each size in front, right-aligned in ten columns; `-` where the format records none. Not with `--json` |
| `--json` | Print [an array](@/docs/json.md#list) with each path, its key and its sizes |

## verify

Checks the archive's names and, if asked, that every file can be read. Warnings are part of the
report; the exit code is 0 unless a file cannot be read or the archive cannot be opened.

| Argument or option | Meaning |
|---|---|
| `<ARCHIVE>` | The archive |
| `--read-payloads` | Read every named file to the end, decompressing it |
| `--json` | Print [the report](@/docs/json.md#verify) |

[Inspecting archives](@/docs/inspecting.md#verify) explains each line.

## diff

Compares two archives entry by entry, by normalized path, and prints how many entries were
added, removed, changed and unchanged.

| Argument or option | Meaning |
|---|---|
| `<OLD>`, `<NEW>` | The two archives, in any formats |
| `--hash` | Compare a 64-bit FNV-1a fingerprint of every file's bytes, not just its sizes |
| `--json` | Print [the report](@/docs/json.md#diff), with every added, removed and changed entry |

## extract

Extracts one entry to a file, or to standard output.

| Argument or option | Meaning |
|---|---|
| `<ARCHIVE>` | The archive |
| `<ENTRY>` | The entry's name, in any case, with `/` or `\`. On Linux and macOS it may be bytes that are not UTF-8 |
| `--entry-hex <HEX>` | The entry's `path_bytes_hex` from `list --json`, instead of `<ENTRY>` |
| `-o`, `--output <DIR>` | Write under `DIR`. Default: the current folder |
| `--flat` | Write the file directly in the output folder, without the archive's folders |
| `--overwrite` | Replace the target if it exists |
| `--skip-existing` | Leave the target if it exists, and count it as skipped |
| `--fsync` | Flush the file and its folders to disk |
| `--json` | Print `{ "extracted": 1, "skipped": 0 }` |
| `--stdout` | Write the entry's bytes to standard output and nothing else. Takes none of `--output`, `--flat`, `--overwrite`, `--skip-existing`, `--fsync` or `--json` |

Exactly one of `<ENTRY>` and `--entry-hex` is required; `--overwrite` and `--skip-existing`
exclude each other. [Extracting](@/docs/extracting.md) covers each choice.

## extract-all

Extracts every named entry, each at its normalized path under the output folder.

| Argument or option | Meaning |
|---|---|
| `<ARCHIVE>` | The archive |
| `-o`, `--output <DIR>` | Write under `DIR`. Default: the current folder |
| `--overwrite` | Replace targets that exist |
| `--skip-existing` | Leave targets that exist, and count them as skipped |
| `--fsync` | Flush every file and its folders to disk |
| `--json` | Print `{ "extracted": N, "skipped": N }` |
| `--dry-run` | Print the [extraction plan](@/docs/json.md#extraction-plan) and write nothing |

Under the default policy, one existing target stops the whole extraction before anything is
written.

## create

Packs a folder, or one file, into a new archive, and prints `files: N`.

| Argument or option | Meaning |
|---|---|
| `<ARCHIVE>` | The archive to write. A file already there is replaced once the new one is complete |
| `<INPUT>` | A folder, whose contents are stored relative to it, or a file, stored under its name |
| `--format <FORMAT>` | Required: `tes3`, `tes4` or `ba2` |
| `--tes4-version <VERSION>` | `oblivion` (the default), `fallout3`, `skyrim` or `skyrim-se`. Only with `--format tes4` |
| `--ba2-kind <KIND>` | `gnrl` (the default), `dx10` or `gnmf`. Only with `--format ba2`; `gnmf` is refused before writing |
| `--ba2-version <VERSION>` | `fallout4` (the default), `starfield` or `fallout4-next-gen`. Only with `--format ba2` |
| `--compress` | Compress every file. Not with `--format tes3` |
| `--follow-symlinks` | Pack what symbolic links point to, instead of refusing them |
| `--fsync` | Flush the archive and its folder to disk |
| `--json` | Print `{ "files": N }` |
| `--dry-run` | Print the [creation plan](@/docs/json.md#creation-plan) and write nothing |

[Creating and updating](@/docs/creating.md) has the table of games and formats.

## add

Writes the archive again with files added or replaced, and prints `files: N`, the new archive's
whole count.

| Argument or option | Meaning |
|---|---|
| `<ARCHIVE>` | The archive to update |
| `<INPUTS>...` | One or more folders or files, laid out as for `create` |
| `-o`, `--output <FILE>` | Write the new archive to `FILE` and leave `<ARCHIVE>` alone. Not `<ARCHIVE>` itself. Give it with a folder, as `./new.bsa`: a bare name that does not exist yet fails |
| `--follow-symlinks` | Pack what symbolic links point to, instead of refusing them |
| `--fsync` | Flush the archive and its folder to disk |
| `--json` | Print `{ "files": N }` |
| `--dry-run` | Print the [update plan](@/docs/json.md#update-plan) and write nothing |

Without `--output`, the new archive replaces `<ARCHIVE>` once it is complete.

## Completions and the manual page

| Option | Prints |
|---|---|
| `--generate-completion <SHELL>` | A completion script for `bash`, `zsh`, `fish`, `powershell` or `elvish` |
| `--generate-manpage` | The manual page, in roff |

Either one runs alone: they exclude each other, and a command given with them is ignored.

```sh
dream_archivetool --generate-completion bash > ~/.local/share/bash-completion/completions/dream_archivetool
dream_archivetool --generate-completion fish > ~/.config/fish/completions/dream_archivetool.fish
dream_archivetool --generate-manpage > ~/.local/share/man/man1/dream_archivetool.1
```

For zsh, save the script as `_dream_archivetool` in a folder on your `$fpath`.

## Where output goes

| Stream | Gets |
|---|---|
| Standard output | The result: the lines each command prints, the JSON, or with `--stdout` the file's bytes |
| Standard error | Errors, as `ERROR: <message>`, and usage mistakes, as `error: ...` with a usage line |

Nothing else is printed: no progress, no banners. A command that writes files prints only its
count when it is done.

Output piped into a program that stops reading early, such as `head`, ends with
`ERROR: I/O error: Broken pipe (os error 32)` on standard error and exit code 1, after the lines
that were read. The lines are right; the message is noise.

## Exit codes

| Code | When |
|---|---|
| `0` | Success, warnings or not. Also `--help`, `--version`, and no command at all |
| `1` | The command failed: a file that cannot be opened or read, an entry not found, an unsafe name, an existing target, a refused rewrite. What was checked before writing, was checked before anything was written |
| `2` | The command line is wrong: an unknown command or option, a missing argument, two options that exclude each other |

## Examples

```sh
# What is in it, for a script.
dream_archivetool list --json Morrowind.bsa > entries.json

# Is every file readable?
dream_archivetool verify Morrowind.bsa --read-payloads

# What changed between two releases of a mod, by content.
dream_archivetool diff MyMod-1.0.bsa MyMod-1.1.bsa --hash

# One texture, to look at.
dream_archivetool extract Morrowind.bsa textures/tx_bc_moss.dds --stdout > moss.dds

# Everything, keeping the files you already extracted.
dream_archivetool extract-all Morrowind.bsa --output Morrowind --skip-existing

# A Skyrim Special Edition archive, compressed.
dream_archivetool create MyMod.bsa MyMod --format tes4 --tes4-version skyrim-se --compress

# A Fallout 4 texture archive.
dream_archivetool create "MyMod - Textures.ba2" MyModTextures --format ba2 --ba2-kind dx10

# New files into a copy, after reading the plan.
dream_archivetool add MyMod.bsa Update --output ./MyMod-new.bsa --dry-run
dream_archivetool add MyMod.bsa Update --output ./MyMod-new.bsa
```
