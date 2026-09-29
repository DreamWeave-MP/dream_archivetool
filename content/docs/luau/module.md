+++
title = "@dream/archivetool"
description = "info, verify, diff, the extraction functions and their plans, create, add and their plans: arguments, option tables, results and errors."
weight = 10

[extra]
kind = "api"
+++

```lua
local tool = require("@dream/archivetool")
```

A frozen table of sixteen functions. Each takes a host path, opens the archive for that one
call, and closes it again; to run several operations on one archive, open it once with
dream_archive and use the [archive methods](@/docs/luau/archive.md) instead. Creating and
updating archives are host-path operations only: there is no `archive:create` or `archive:add`.

```lua
local tool = require("@dream/archivetool")

local info = tool.info("Morrowind.bsa")
print(info.format, info.fileCount, info.rewritable)

local plan = tool.planExtractAll("Morrowind.bsa", { output = "out", overwrite = "skip" })
for _, row in plan.entries do
    if row.action == "conflict" then
        print("would stop at", row.target)
    end
end
```

## info

{{ api_signature(value="info(path: string) -> ArchiveInfo") }}

The archive's format, counts and rewritability, as a plain table. `archive:toolInfo()` returns
the same for an opened archive.

| Field | Is |
|---|---|
| `path` | `path` as given |
| `format` | `"bsaTes3"`, `"bsaTes4"` or `"ba2"` |
| `fileCount`, `namedEntryCount` | Every entry, and the entries with a name |
| `hasUnnameableEntries` | Whether any entry has only a hash |
| `rewritable`, `rewriteBlocker` | Whether `add` would rewrite it, and why not; `rewriteBlocker` is `nil` when it would |
| `tes4` | TES4 only: `version`, `archiveTypes`, `archiveTypesBits`, `archiveFlags`, `archiveFlagsBits`, `unsupportedArchiveFlagsBits`, `nameMode` |
| `ba2` | BA2 only: `version`, `payloadFormat`, `compressionFormat`, `strings` |

The nested fields mean what they do in [`info --json`](@/docs/json.md#info), with camelCase
values: `archiveFlags` holds `"directoryStrings"`, `"fileStrings"`, `"compressed"` and
`"embeddedFileNames"`, and `nameMode` is `"strings"`, `"embedded"`, `"stringsAndEmbedded"` or
`"hashOnly"`.

## verify and diff

{{ api_signature(value="verify(path: string, options: { readPayloads: boolean? }?) -> VerifyReport") }}

Checks the archive's names and, with `readPayloads`, reads every named file to the end. A file
that cannot be read raises an error; everything else is in the
[report](@/docs/luau/results.md#verifyreport).

{{ api_signature(value="diff(old: string, new: string, options: { fingerprintPayloads: boolean? }?) -> DiffReport") }}

Compares two archives by normalized path. With `fingerprintPayloads`, it reads both in full and
compares a 64-bit FNV-1a fingerprint of each file. Raises an error for an archive with unnamed
entries or duplicate normalized names. The [report](@/docs/luau/results.md#diffreport) lists
what changed.

## Extracting

{{ api_signature(value="extract(path: string, entry: string | Entry, options: ExtractOptions?) -> { extracted: number, skipped: number }") }}

One entry, by name in any spelling or by an entry handle from `archive:entries()`.

{{ api_signature(value="extractByPathHex(path: string, pathBytesHex: string, options: ExtractOptions?) -> { extracted: number, skipped: number }") }}

{{ api_signature(value="extractHex(path: string, pathBytesHex: string, options: ExtractOptions?) -> { extracted: number, skipped: number }") }}

One entry, by its `pathBytesHex` key from a report or plan. `extractHex` is the same function
under an older name.

{{ api_signature(value="extractMany(path: string, entries: { string | Entry } | Entries, options: ExtractOptions?) -> { extracted: number, skipped: number }") }}

{{ api_signature(value="extractManyByPathHex(path: string, pathBytesHex: { string }, options: ExtractOptions?) -> { extracted: number, skipped: number }") }}

Several entries in one pass over one opened archive. `entries` is an array of names or entry
handles, or a whole entries view such as `archive:entries()`. Every entry must exist, every
target must be safe and distinct, and under `overwrite = "fail"` none may exist, or nothing is
written.

{{ api_signature(value="planExtract(path: string, entries: { string | Entry } | Entries, options: ExtractOptions?) -> ExtractPlan") }}

{{ api_signature(value="planExtractByPathHex(path: string, pathBytesHex: { string }, options: ExtractOptions?) -> ExtractPlan") }}

What `extractMany` would do, without writing: one row per entry with its target and action.
An entry the archive does not have is an error, as in `extractMany`; a target that exists is a
`"conflict"` row, not an error.

{{ api_signature(value="extractAll(path: string, options: ExtractAllOptions?) -> { extracted: number, skipped: number }") }}

{{ api_signature(value="planExtractAll(path: string, options: ExtractAllOptions?) -> ExtractPlan") }}

Every named entry, as [`extract-all`](@/docs/extracting.md#everything) does, and its plan. An
archive with unnamed entries is refused.

`ExtractOptions`, all optional:

| Option | Default | Is |
|---|---|---|
| `output` | `"."` | The folder to write under |
| `overwrite` | `"fail"` | `"fail"`, `"overwrite"` or `"skip"`, as in [Files you already have](@/docs/extracting.md#files-you-already-have) |
| `preservePaths` | `true` | Keep the archive's folders; `false` writes each file directly in `output`, as `--flat` does |
| `fsync` | `false` | Flush each file and its folders to disk |

`ExtractAllOptions` are the same without `preservePaths`.

## Creating and updating

{{ api_signature(value="create(output: string, input: string, options: CreateOptions?) -> { files: number }") }}

{{ api_signature(value="planCreate(output: string, input: string, options: CreateOptions?) -> CreatePlan") }}

Packs the folder or file `input` into a new archive at `output`, as
[`create`](@/docs/creating.md#create) does, and its plan.

| Option | Default | Is |
|---|---|---|
| `format` | `"tes3"` | `"tes3"` or `"bsaTes3"`, `"tes4"` or `"bsaTes4"`, `"ba2"` |
| `tes4Version` | `"oblivion"` | `"oblivion"`, `"fallout3"`, `"skyrim"`, `"skyrimSe"` or `"sse"`. TES4 only |
| `ba2Kind` | `"gnrl"` | `"gnrl"`, `"dx10"` or `"gnmf"`, which is refused. BA2 only |
| `ba2Version` | `"fallout4"` | `"fallout4"`, `"starfield"` or `"fallout4NextGen"`. BA2 only |
| `compress` | `false` | Compress every file. Not with TES3 |
| `fsync` | `false` | Flush the archive and its folder to disk |
| `followSymlinks` | `false` | Pack what symbolic links point to, instead of refusing them |

Unlike the command line, `format` has a default. An option that belongs to another format is an
error, as `ba2Kind is not valid with format bsaTes3`, and so is `compress` with TES3. A symbolic
link among the inputs is refused with `refusing to follow symlink input path: <path>; set
followSymlinks to opt in`.

{{ api_signature(value="add(archive: string, options: AddOptions) -> { files: number }") }}

{{ api_signature(value="planAdd(archive: string, options: AddOptions) -> AddPlan") }}

Writes `archive` again with the inputs added or replacing entries, as
[`add`](@/docs/creating.md#add) does, and its plan. `files` counts every file in the new archive.

| Option | Default | Is |
|---|---|---|
| `inputs` | required | An array of one or more host paths, folders or files |
| `output` | the archive | Where to write the new archive. Not `archive` itself |
| `fsync` | `false` | Flush the archive and its folder to disk |
| `followSymlinks` | `false` | Pack what symbolic links point to, instead of refusing them |

## Errors

Every failure raises a string error; nothing is returned. The option and argument errors name
the function:

| Mistake | Error |
|---|---|
| An unknown option | `extract: unknown option 'overwirte'; known options are fsync, output, overwrite, preservePaths` |
| An option of the wrong type | `extract.output: expected string, got number` |
| An unknown value | `extract.overwrite: unknown overwrite mode: explode` |
| An option for another format | `ba2Kind is not valid with format bsaTes3`, `compress is not valid with format bsaTes3` |
| A bad `pathBytesHex` | `extractByPathHex: invalid pathBytesHex: archive error: archive path hex contains a non-hexadecimal digit` |
| An entries array with holes | `planExtract.entries must be a dense 1-based array` |
| A wrong entry | `extractMany.entries[1]: expected an archive path or an entry handle, got number` |
| No inputs | `add requires at least one input path` |
| A missing argument | `dream.archivetool.extract: bad argument count (expected at least 2, got 1)` |

The operations' own failures read as the command line's do, without `ERROR:`:
`archive entry not found: missing.dds`, `target already exists: o/readme.txt`,
`unsafe archive path: ../readme.txt`, and `archive error: ...` for the rest, such as
`archive error: failed to open archive 'nope.bsa': No such file or directory (os error 2)`.
