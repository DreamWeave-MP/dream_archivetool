+++
title = "Reports and plans"
description = "VerifyReport, DiffReport, ExtractPlan, CreatePlan and AddPlan: their fields, the row views that take #, [i] and for, the row types, and :toTable()."
weight = 30

[extra]
kind = "api"
+++

`verify` and `diff` return reports; the `plan` functions return plans. Both are userdata over
one copy of the result: fields are read as properties, and lists of rows are views that build a
row only when a script asks for it. Nothing is copied into tables unless the script calls
`:toTable()`.

```lua
local dreamArchive = require("@dream/archive")

local old = dreamArchive.openPath("MyMod.bsa")
local new = dreamArchive.openPath("MyMod-2.bsa")
local diff = old:diff(new, { fingerprintPayloads = true })

print(#diff.added, #diff.removed, #diff.changed, diff.unchanged)
for _, change in diff.changed do
    if change.old.payloadFingerprint ~= change.new.payloadFingerprint then
        print(change.path, change.old.size, "->", change.new.size)
    end
end
```

## Views

Every list of rows is a view:

| Takes | Gives |
|---|---|
| `#view` | The number of rows |
| `view[i]` | Row `i`, from 1, or `nil` past the end |
| `for i, row in view do` | Each row in order, with its index |
| `view:toTable()` | A plain array of the row handles |

A view is not a table: `ipairs`, `pairs` and `table.insert` do not apply to it. A row is a handle
into the report it came from and keeps that report alive; its fields are read from it on access.

## VerifyReport

`dream.archivetool.VerifyReport`, from `verify`:

| Field | Type | Is |
|---|---|---|
| `path` | `string` | The archive's path, or `<memory>` |
| `format` | `string` | `"bsaTes3"`, `"bsaTes4"` or `"ba2"` |
| `fileCount`, `namedEntryCount`, `unnameableEntries` | `number` | Every entry, the named ones, and the ones with only a hash |
| `rewritable` | `boolean` | Whether `add` would rewrite the archive |
| `rewriteBlocker` | `string?` | Why not |
| `duplicateNormalizedPaths` | view of `PathIssue` | Entries whose normalized name an earlier entry already has |
| `unsafePaths` | view of `PathIssue` | Names extraction would refuse |
| `payloadsRead` | `number?` | With `readPayloads`, how many files were read; `nil` without it or when duplicates made reading by name ambiguous |
| `warnings` | view of `string` | The warnings, in the command line's words |

`PathIssue` has `path`, `pathBytesHex`, and for a duplicate, `rawPathBytesHex` (this entry's
stored bytes) and `collidingRawPathBytesHex` (the earlier entry's). In `unsafePaths`, `path` and
`pathBytesHex` are the name as stored, not normalized.

## DiffReport

`dream.archivetool.DiffReport`, from `diff`:

| Field | Type | Is |
|---|---|---|
| `old`, `new` | `string` | The two archives' paths |
| `comparison` | `string` | `"metadataOnly"`, or `"payloadFingerprint"` with `fingerprintPayloads` |
| `fingerprintPayloads` | `boolean` | Whether fingerprints were computed |
| `added`, `removed` | view of `DiffEntry` | Entries only in `new`, and only in `old` |
| `changed` | view of `DiffChange` | Entries in both that differ |
| `unchanged` | `number` | Entries in both that are the same |

`DiffEntry` has `path`, `pathBytesHex`, `size`, `compressedSize` and `payloadFingerprint`.
`DiffChange` has `path`, `pathBytesHex`, and `old` and `new`, each a `DiffState` with `size`,
`compressedSize` and `payloadFingerprint`. Those three are `integer?`: `nil` where the format
records no size, the entry is not compressed, or no fingerprint was asked for.
`payloadFingerprint` carries all 64 bits of the FNV-1a hash.

Sizes and fingerprints are Luau `integer` values, which take `==` and `tostring`, and nothing
else directly: `<`, `+` and `tonumber` do not apply to them. Compare and convert them with Luau's
`integer` library, as `integer.lt(a.size, b.size)` or `integer.tonumber(a.size)`.

## ExtractPlan

`dream.archivetool.ExtractPlan`, from `planExtract`, `planExtractByPathHex` and
`planExtractAll`:

| Field | Type | Is |
|---|---|---|
| `operation` | `string` | `"extract"` for chosen entries, `"extractAll"` for the whole archive |
| `archive` | `string` | The archive's path, or `<memory>` |
| `output` | `string` | The output folder |
| `entries` | view of `ExtractPlanRow` | One row per entry |

`ExtractPlanRow` has `action`, `path`, `pathBytesHex` and `target`, the file it would write.
`action` is `"extract"`, `"overwrite"`, `"skip"` or `"conflict"`, as on
[Extracting](@/docs/extracting.md#dry-runs).

## CreatePlan and AddPlan

`dream.archivetool.CreatePlan`, from `planCreate`, has `operation` (`"create"`), `format`,
`output`, `files` and `entries`. `dream.archivetool.AddPlan`, from `planAdd`, has `operation`
(`"add"`), `archive`, `output`, `format`, `files`, `added`, `replaced`, `preserved` and
`entries`. Both `entries` are views of `ArchivePlanRow`:

| Field | Type | Is |
|---|---|---|
| `action` | `string` | `"add"`, `"replace"` or `"preserve"` |
| `source` | `string?` | The file it comes from; `nil` for a kept entry |
| `path`, `pathBytesHex` | `string` | Its name in the archive |
| `size` | `integer?` | The source file's size; `nil` for a kept entry |

The counts mean what they do in [the update plan](@/docs/json.md#update-plan).

## toTable

`report:toTable()` and `plan:toTable()` return the whole result as nested plain tables, the
shape 0.2 returned: the same field names, rows as tables instead of handles, absent fields left
out. Sizes and fingerprints are integers there too. It costs a table per row; use it to hand a
result to code that expects tables, or to keep one after changing it.

```lua
local tool = require("@dream/archivetool")

local plan = tool.planCreate("New.bsa", "MyMod", { format = "tes3" })
local t = plan:toTable()
assert(t.operation == "create" and t.files == #t.entries)
table.sort(t.entries, function(a, b) return integer.gt(a.size, b.size) end)
print("largest:", t.entries[1].path)
```

`tostring` of a report or plan names it, as `dream.archivetool.ExtractPlan(MyMod.bsa, 3 entries)`.
