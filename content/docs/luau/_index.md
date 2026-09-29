+++
title = "Luau API"
description = "The @dream/archivetool module, the methods it adds to dream_archive's archives, the reports and plans they return, and the extension a host composes."
template = "docs/section.html"
page_template = "docs/page.html"
sort_by = "weight"
weight = 100

[extra]
kind = "api"
hide_child_cards = true
+++

With the `luau` feature, dream_archivetool is an l3i extension, `dream.archivetool`, built on
dream_archive's `dream.archive`. Scripts get the same operations as the command line in two
places:

```lua
local dreamArchive = require("@dream/archive")
local tool = require("@dream/archivetool")

-- By path: the function opens the archive for this one call.
print(tool.info("Morrowind.bsa").fileCount)

-- On an archive a script already has: the methods run on that handle.
local archive = dreamArchive.openPath("Morrowind.bsa")
local plan = archive:planExtractAll({ output = "out" })
print(#plan.entries, plan.entries[1].action)
```

| Page | Covers |
|---|---|
| [@dream/archivetool](@/docs/luau/module.md) | The sixteen functions that take host paths, including `create` and `add`, and their options |
| [Archive methods](@/docs/luau/archive.md) | `verify`, `diff`, `extract` and the rest, on every `dream.archive.Archive` |
| [Reports and plans](@/docs/luau/results.md) | What `verify`, `diff` and the plans return: fields, row views and `:toTable()` |
| [Type definitions](@/docs/luau/types.md) | The `.d.luau` the plan generates for the extension |
| [ArchivetoolExtension](@/docs/luau/extension.md) | The Rust side: the extension, its constants and its userdata types |

Opening archives, listing their entries and reading bytes are dream_archive's; its
[documentation](https://DreamWeave-MP.github.io/dream_archive/) covers `dreamArchive.openPath`,
`openBytes`, entries and the builders. [Embedding Luau](@/docs/luau-hosts.md) walks through the
host setup.

## Conventions

- **Names** are camelCase: functions, methods, option keys, fields, and string values such as
  `"bsaTes4"` or `"payloadFingerprint"`.
- **Host paths** are UTF-8 strings: archives, output folders, inputs.
- **Archive entries** are byte strings in any spelling, entry handles from `archive:entries()`,
  or `pathBytesHex` keys for the `ByPathHex` forms. See [Archive paths](@/docs/paths.md).
- **Option tables are strict.** An unknown or misspelled key is an error that lists the known
  ones; a missing table, or `nil`, means every default.
- **Sizes** (`size`, `compressedSize`) and **fingerprints** (`payloadFingerprint`) are Luau
  `integer` values: compare them with `==` or the `integer` library, not `<` or `+`, and write
  a literal as `4096i`. dream_archive's `entry.size` is a number, which never equals an integer;
  [Sizes are integers](@/docs/luau/results.md#sizes-are-integers) has the conversions. **Counts**
  (`fileCount`, `extracted`, `files`) are numbers.
- **Errors** are raised, never returned. A failed call has written nothing that its checks could
  have prevented.
