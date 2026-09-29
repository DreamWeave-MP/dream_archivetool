+++
title = "Type definitions"
description = "The Luau type definitions the plan generates for @dream/archivetool, its report and plan types, and the members it adds to dream_archive's archive type."
weight = 40

[extra]
kind = "api"
+++

Every function, method and field of the extension carries a Luau type. A host gets the
definitions from its plan with `plan.type_definitions()`, ready to save as a `.d.luau` file for
an editor's language server; [Embedding Luau](@/docs/luau-hosts.md#types-for-editors-and-checks)
shows how. The text covers every extension in the plan, dream_archive's included. What follows
is the part dream.archivetool contributes, exactly as a plan of dream_archive and
dream_archivetool generates it.

Names are spelled with underscores: `dream.archivetool.VerifyReport` is
`dream_archivetool_VerifyReport`, and dream_archive's entry handle is `dream_archive_Entry`.

## The module

```luau
-- module @dream/archivetool (provided by dream.archivetool)
-- Archive policy: safe extraction, plans, verification, diffs, creation, and updates by host path.
export type Module__dream_archivetool = {
    info: (path: string) -> { path: string, format: string, fileCount: number, namedEntryCount: number, hasUnnameableEntries: boolean, rewritable: boolean, rewriteBlocker: string?, tes4: { version: string, archiveTypes: string, archiveTypesBits: number, archiveFlags: { string }, archiveFlagsBits: number, unsupportedArchiveFlagsBits: number, nameMode: string }?, ba2: { version: string, payloadFormat: string, compressionFormat: string, strings: boolean }? },
    verify: (path: string, options: { readPayloads: boolean? }?) -> dream_archivetool_VerifyReport,
    diff: (old: string, new: string, options: { fingerprintPayloads: boolean? }?) -> dream_archivetool_DiffReport,
    extract: (path: string, entry: string | dream_archive_Entry, options: { output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?) -> { extracted: number, skipped: number },
    -- Same as extractByPathHex.
    extractHex: (path: string, pathBytesHex: string, options: { output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?) -> { extracted: number, skipped: number },
    extractByPathHex: (path: string, pathBytesHex: string, options: { output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?) -> { extracted: number, skipped: number },
    extractMany: (path: string, entries: { string | dream_archive_Entry } | dream_archive_Entries, options: { output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?) -> { extracted: number, skipped: number },
    extractManyByPathHex: (path: string, pathBytesHex: { string }, options: { output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?) -> { extracted: number, skipped: number },
    planExtract: (path: string, entries: { string | dream_archive_Entry } | dream_archive_Entries, options: { output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?) -> dream_archivetool_ExtractPlan,
    planExtractByPathHex: (path: string, pathBytesHex: { string }, options: { output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?) -> dream_archivetool_ExtractPlan,
    extractAll: (path: string, options: { output: string?, overwrite: string?, fsync: boolean? }?) -> { extracted: number, skipped: number },
    planExtractAll: (path: string, options: { output: string?, overwrite: string?, fsync: boolean? }?) -> dream_archivetool_ExtractPlan,
    create: (output: string, input: string, options: { format: string?, tes4Version: string?, ba2Kind: string?, ba2Version: string?, compress: boolean?, fsync: boolean?, followSymlinks: boolean? }?) -> { files: number },
    planCreate: (output: string, input: string, options: { format: string?, tes4Version: string?, ba2Kind: string?, ba2Version: string?, compress: boolean?, fsync: boolean?, followSymlinks: boolean? }?) -> dream_archivetool_CreatePlan,
    add: (archive: string, options: { inputs: { string }, output: string?, fsync: boolean?, followSymlinks: boolean? }) -> { files: number },
    planAdd: (archive: string, options: { inputs: { string }, output: string?, fsync: boolean?, followSymlinks: boolean? }) -> dream_archivetool_AddPlan,
}
```

A host that exposes the [compatibility global](@/docs/luau-hosts.md#the-dreamarchivetool-global)
also gets this line:

```luau
declare dreamArchivetool: Module__dream_archivetool
```

## The archive methods

The members dream.archivetool adds to dream_archive's `dream_archive_Archive`, inside that type's
declaration:

```luau
    function toolInfo(self): { path: string, format: string, fileCount: number, namedEntryCount: number, hasUnnameableEntries: boolean, rewritable: boolean, rewriteBlocker: string?, tes4: { version: string, archiveTypes: string, archiveTypesBits: number, archiveFlags: { string }, archiveFlagsBits: number, unsupportedArchiveFlagsBits: number, nameMode: string }?, ba2: { version: string, payloadFormat: string, compressionFormat: string, strings: boolean }? }
    function verify(self, options: { readPayloads: boolean? }?): dream_archivetool_VerifyReport
    function diff(self, other: dream_archive_Archive, options: { fingerprintPayloads: boolean? }?): dream_archivetool_DiffReport
    function extract(self, entry: string | dream_archive_Entry, options: { output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?): { extracted: number, skipped: number }
    function extractMany(self, entries: { string | dream_archive_Entry } | dream_archive_Entries, options: { output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?): { extracted: number, skipped: number }
    function planExtract(self, entries: { string | dream_archive_Entry } | dream_archive_Entries, options: { output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?): dream_archivetool_ExtractPlan
    function extractByPathHex(self, pathBytesHex: string, options: { output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?): { extracted: number, skipped: number }
    function extractManyByPathHex(self, pathBytesHex: { string }, options: { output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?): { extracted: number, skipped: number }
    function planExtractByPathHex(self, pathBytesHex: { string }, options: { output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?): dream_archivetool_ExtractPlan
    function extractAll(self, options: { output: string?, overwrite: string?, fsync: boolean? }?): { extracted: number, skipped: number }
    function planExtractAll(self, options: { output: string?, overwrite: string?, fsync: boolean? }?): dream_archivetool_ExtractPlan
```

## Reports, plans and rows

Every report and plan type, each view with its row type, and each row, in the order the plan
writes them. `__len`, the `[number]` indexer and `__iter` are what let strict scripts measure,
index and iterate a view.

```luau
-- dream.archivetool.AddPlan (owned by dream.archivetool; untagged)
-- What adding to an archive would rewrite.
declare extern type dream_archivetool_AddPlan with
    operation: string
    archive: string
    output: string
    format: string
    files: number
    added: number
    replaced: number
    preserved: number
    entries: dream_archivetool_ArchivePlanRows
    function toTable(self): { [string]: any }
end

-- dream.archivetool.ArchivePlanRow (owned by dream.archivetool; tag 4)
-- One planned archive member of a create or add plan.
declare extern type dream_archivetool_ArchivePlanRow with
    action: string
    source: string?
    path: string
    pathBytesHex: string
    size: integer?
end

-- dream.archivetool.ArchivePlanRows (owned by dream.archivetool; untagged)
declare extern type dream_archivetool_ArchivePlanRows with
    function toTable(self): { dream_archivetool_ArchivePlanRow }
    function __len(self): number
    [number]: dream_archivetool_ArchivePlanRow?
    function __iter(self): (({}, number) -> (number?, dream_archivetool_ArchivePlanRow), {}, number)
end

-- dream.archivetool.CreatePlan (owned by dream.archivetool; untagged)
-- What creating an archive would write.
declare extern type dream_archivetool_CreatePlan with
    operation: string
    format: string
    output: string
    files: number
    entries: dream_archivetool_ArchivePlanRows
    function toTable(self): { [string]: any }
end

-- dream.archivetool.DiffChange (owned by dream.archivetool; tag 5)
-- An entry present on both sides with a difference.
declare extern type dream_archivetool_DiffChange with
    path: string
    pathBytesHex: string
    old: dream_archivetool_DiffState
    new: dream_archivetool_DiffState
end

-- dream.archivetool.DiffChanges (owned by dream.archivetool; untagged)
declare extern type dream_archivetool_DiffChanges with
    function toTable(self): { dream_archivetool_DiffChange }
    function __len(self): number
    [number]: dream_archivetool_DiffChange?
    function __iter(self): (({}, number) -> (number?, dream_archivetool_DiffChange), {}, number)
end

-- dream.archivetool.DiffEntries (owned by dream.archivetool; untagged)
declare extern type dream_archivetool_DiffEntries with
    function toTable(self): { dream_archivetool_DiffEntry }
    function __len(self): number
    [number]: dream_archivetool_DiffEntry?
    function __iter(self): (({}, number) -> (number?, dream_archivetool_DiffEntry), {}, number)
end

-- dream.archivetool.DiffEntry (owned by dream.archivetool; tag 6)
-- An entry present on one side of a diff.
declare extern type dream_archivetool_DiffEntry with
    path: string
    pathBytesHex: string
    size: integer?
    compressedSize: integer?
    payloadFingerprint: integer?
end

-- dream.archivetool.DiffReport (owned by dream.archivetool; untagged)
-- Two archives compared by normalized path and metadata.
declare extern type dream_archivetool_DiffReport with
    old: string
    new: string
    comparison: string
    fingerprintPayloads: boolean
    added: dream_archivetool_DiffEntries
    removed: dream_archivetool_DiffEntries
    changed: dream_archivetool_DiffChanges
    unchanged: number
    function toTable(self): { [string]: any }
end

-- dream.archivetool.DiffState (owned by dream.archivetool; untagged)
-- One side's metadata for a changed entry.
declare extern type dream_archivetool_DiffState with
    size: integer?
    compressedSize: integer?
    payloadFingerprint: integer?
end

-- dream.archivetool.ExtractPlan (owned by dream.archivetool; untagged)
-- What an extraction would write, per entry.
declare extern type dream_archivetool_ExtractPlan with
    operation: string
    archive: string
    output: string
    entries: dream_archivetool_ExtractPlanRows
    function toTable(self): { [string]: any }
end

-- dream.archivetool.ExtractPlanRow (owned by dream.archivetool; tag 7)
-- One planned extraction target.
declare extern type dream_archivetool_ExtractPlanRow with
    action: string
    path: string
    pathBytesHex: string
    target: string
end

-- dream.archivetool.ExtractPlanRows (owned by dream.archivetool; untagged)
declare extern type dream_archivetool_ExtractPlanRows with
    function toTable(self): { dream_archivetool_ExtractPlanRow }
    function __len(self): number
    [number]: dream_archivetool_ExtractPlanRow?
    function __iter(self): (({}, number) -> (number?, dream_archivetool_ExtractPlanRow), {}, number)
end

-- dream.archivetool.PathIssue (owned by dream.archivetool; tag 8)
-- A duplicate or unsafe archive path.
declare extern type dream_archivetool_PathIssue with
    path: string
    pathBytesHex: string
    rawPathBytesHex: string?
    collidingRawPathBytesHex: string?
end

-- dream.archivetool.PathIssues (owned by dream.archivetool; untagged)
declare extern type dream_archivetool_PathIssues with
    function toTable(self): { dream_archivetool_PathIssue }
    function __len(self): number
    [number]: dream_archivetool_PathIssue?
    function __iter(self): (({}, number) -> (number?, dream_archivetool_PathIssue), {}, number)
end

-- dream.archivetool.VerifyReport (owned by dream.archivetool; untagged)
-- Archive index health: duplicate and unsafe paths, optional payload reads.
declare extern type dream_archivetool_VerifyReport with
    path: string
    format: string
    fileCount: number
    namedEntryCount: number
    unnameableEntries: number
    rewritable: boolean
    rewriteBlocker: string?
    duplicateNormalizedPaths: dream_archivetool_PathIssues
    unsafePaths: dream_archivetool_PathIssues
    payloadsRead: number?
    warnings: dream_archivetool_Warnings
    -- The report as the nested table 0.2 returned (sizes and fingerprints as integers).
    function toTable(self): { [string]: any }
end

-- dream.archivetool.Warnings (owned by dream.archivetool; untagged)
declare extern type dream_archivetool_Warnings with
    function toTable(self): { string }
    function __len(self): number
    [number]: string?
    function __iter(self): (({}, number) -> (number?, string), {}, number)
end
```
