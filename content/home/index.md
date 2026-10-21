+++
title = "dream-archivetool"
description = "Inspects, extracts, creates and updates Bethesda BSA and BA2 archives over dream-archive, as a Jess package and command in Luau."
date = 2026-10-21

[taxonomies]
tags = ["Jess", "Luau", "OpenMW"]

[extra]
sections = ["overview", "releases", "credits"]
+++

Inspects, extracts, creates and updates Bethesda BSA and BA2 archives over dream-archive, as a Jess package and command in Luau.

<!-- more -->

A [Jess](https://github.com/DreamWeave-MP/jess) package, `org.dreamweave.dream-archivetool`. Add it to a `jess.toml` and
`jess workspace sync` pins it from this site:

```toml
[dependencies]
"org.dreamweave.dream-archivetool" = "0.1"
```

Every release below is the package's own archive, with its content digest beside the archive's
SHA-256; `jess.lock` pins the first and checks both.
