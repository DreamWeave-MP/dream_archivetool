+++
title = "Archive methods"
description = "toolInfo, verify, diff, extract, extractMany, planExtract, the ByPathHex forms, extractAll and planExtractAll, on every dream.archive.Archive."
weight = 20

[extra]
kind = "api"
+++

The extension adds eleven methods to `dream.archive.Archive`, the type dream_archive's
`dreamArchive.openPath` and `openBytes` return. They are members of that type, not of a wrapper:
every archive has them, whichever extension a script required first, and they run against the
handle the script already holds instead of opening the file again.

```lua
local dreamArchive = require("@dream/archive")

local archive = dreamArchive.openPath("Morrowind.bsa")
local report = archive:verify()
print(report.fileCount, report.rewritable, #report.unsafePaths)

-- Every icon, in one pass, into out/. An entry's path is spelled as the archive stores it,
-- icons\tx_goldicon.dds in Morrowind.bsa, so match either slash.
local icons = {}
for _, entry in archive:entries() do
    local path = entry.path
    if path and string.find(string.lower(path), "^icons[/\\]") then
        table.insert(icons, entry)
    end
end
local summary = archive:extractMany(icons, { output = "out" })
print(summary.extracted, "icons")
```

The script never requires `@dream/archivetool`: the methods are there as soon as the host has
composed the extension.

The methods take the same options, return the same results and raise the same errors as the
[module functions](@/docs/luau/module.md) of the same names; only the archive comes from the
handle, and errors name the method, as `archive:extract: ...`. Reports and plans label the
archive with the path it was opened from, or `<memory>` for one opened from bytes.

## toolInfo

{{ api_signature(value="archive:toolInfo() -> ArchiveInfo") }}

The table [`info`](@/docs/luau/module.md#info) returns: format, counts, rewritability and header
facts. dream_archive's own `archive:info()` is a different, BA2-only table.

## verify and diff

{{ api_signature(value="archive:verify(options: { readPayloads: boolean? }?) -> VerifyReport") }}

{{ api_signature(value="archive:diff(other: Archive, options: { fingerprintPayloads: boolean? }?) -> DiffReport") }}

`other` is another opened archive, of any format; this archive is the old side.

## Extracting

{{ api_signature(value="archive:extract(entry: string | Entry, options: ExtractOptions?) -> { extracted: number, skipped: number }") }}

{{ api_signature(value="archive:extractByPathHex(pathBytesHex: string, options: ExtractOptions?) -> { extracted: number, skipped: number }") }}

{{ api_signature(value="archive:extractMany(entries: { string | Entry } | Entries, options: ExtractOptions?) -> { extracted: number, skipped: number }") }}

{{ api_signature(value="archive:extractManyByPathHex(pathBytesHex: { string }, options: ExtractOptions?) -> { extracted: number, skipped: number }") }}

{{ api_signature(value="archive:planExtract(entries: { string | Entry } | Entries, options: ExtractOptions?) -> ExtractPlan") }}

{{ api_signature(value="archive:planExtractByPathHex(pathBytesHex: { string }, options: ExtractOptions?) -> ExtractPlan") }}

{{ api_signature(value="archive:extractAll(options: ExtractAllOptions?) -> { extracted: number, skipped: number }") }}

{{ api_signature(value="archive:planExtractAll(options: ExtractAllOptions?) -> ExtractPlan") }}

An entry handle, or an entries view such as `archive:entries()`, names its entry by its path. A
handle with no path, from a hash-only archive, is refused:

```text
archive:extract: entry 1 has no path; hash-only entries are not extraction targets
```

Handles from another archive are read by their path, and looked up in this one.

## Byte snapshots

An archive from `dreamArchive.openBytes` is a copy of the bytes it was given. Its methods read
that copy, so they are unaffected by later changes to the file it came from, and its reports say
`<memory>` where a path would be.
