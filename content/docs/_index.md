+++
title = "Documentation"
description = "How to inspect, extract, create and update Bethesda archives with dream_archivetool, what each command checks and refuses, and its Rust and Luau API."
template = "docs/section.html"
page_template = "docs/page.html"
sort_by = "weight"

[extra]
docs_root = true
docs_project_name = "dream_archivetool"
docs_short_title = "dream_archivetool docs"
docs_project_path = "@/home/index.md"
docs_repository_url = "https://github.com/DreamWeave-MP/dream_archivetool/tree/main/content/docs"
docs_sidebar_label = "Documentation"
hide_child_cards = true
kind = "guide"
+++

dream_archivetool reads and writes the archives Bethesda's games load their assets from. The
command line, the Rust library and the Luau extension run the same code, so everything here about
what a command checks, writes and refuses applies to all three.

## Learn it

- **[Start here](@/docs/start-here.md)**: download it, look inside an archive, take files out,
  and pack a folder into a new one.

## Use it

- **[Inspecting archives](@/docs/inspecting.md)**: what is inside, whether it is healthy, and
  what changed between two archives.
- **[Extracting](@/docs/extracting.md)**: one file, a list, or everything; where files land,
  what happens to files you already have, and the paths it refuses.
- **[Creating and updating](@/docs/creating.md)**: packing a folder, choosing the format, adding
  to an archive, and what it refuses to rewrite.
- **[Embedding Luau](@/docs/luau-hosts.md)**: giving scripts `@dream/archivetool` through l3i.

## Look it up

- **[Command line](@/docs/cli.md)**: every command and option, where output goes, and the exit
  codes.
- **[JSON output](@/docs/json.md)**: every shape `--json` and `--dry-run` print.
- **[Archive paths](@/docs/paths.md)**: how entry names are normalized, what `path_bytes_hex`
  is, and which names are unsafe.
- **[Formats](@/docs/formats.md)**: the BSA and BA2 variants it reads and writes, and what an
  update keeps.
- **[Platforms and performance](@/docs/compatibility.md)**: what each download is, building it
  yourself, the license, what is tested, and what the operations cost.
- **[Rust API](@/docs/api/_index.md)**: `ArchiveTool`, the options, the reports and the plans.
- **[Luau API](@/docs/luau/_index.md)**: the `@dream/archivetool` module and the methods it adds
  to dream_archive's archives.
