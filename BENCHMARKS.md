# Archive tool benchmarks

`bench/bench.jsl` times the package through what it exports: the reports (`info`, `list`,
`verify`, `diff`), the plans, extraction, creating and adding to archives, and the command line
with what it prints. It builds its own archives (a TES3 BSA of 4,000 members, another of 3,800 to
compare it with, one of 1,000 to extract and pack, and a BA2 of 1,000) and writes only below the
scratch folder it is given, which it clears when it is done. Each workload is warmed up, sized to
a batch of about a tenth of a second, and reported as the median of seven batches, with a full
collection between them. Every result is checked against what the inputs say it must be: counts,
byte totals, files read back, an archive packed from extracted files against the archive they
came out of, and the command line's standard output against a pinned SHA-256.

```sh
jess run --package jpk/dream-archivetool --package-path jpk \
  --capability filesystem.read --capability filesystem.write \
  jpk/dream-archivetool/bench/bench.jsl <scratch folder> [filter]
```

## The move onto l3i 2.0, base against head

- Machine: Intel Core i7-10870H @ 2.20 GHz, 16 threads, shared with other agents' tests while
  this ran (load average 2.7 when it started, 1.5 when it ended).
- Date: 2026-10-12.
- Binary: the verified release build of Jess 0.1.0 on l3i 2.0.2 (Luau 0.742), the same one for
  both sides.
- Base: `73e4749`, the package before the move, running this bench as it is at head. Head:
  `6f53875`.
- Protocol: the whole bench against base, then head, three times over, back to back under one
  lock; each cell is the median of its three runs. A difference under 5 percent is called no
  change.

| Workload | Base, µs | Head, µs | Base / head |
|---|---:|---:|---:|
| info: ba2, 1000 entries | 227 | 222 | no change |
| list: tes3, 4000 entries | 33833 | 3888 | 8.70x |
| list: ba2, 1000 entries | 9037 | 1306 | 6.92x |
| verify: tes3, 4000 entries | 9753 | 4240 | 2.30x |
| verify: tes3, 4000 entries, payloads read | 11678 | 5851 | 2.00x |
| diff: tes3, 4000 against 3800, by size | 17738 | 8823 | 2.01x |
| diff: tes3, 4000 against 3800, by fingerprint | 27223 | 15206 | 1.79x |
| planExtractAll: tes3, 4000 entries | 358633 | 11700 | 30.65x |
| planExtract: 1000 of 4000, by path | 77621 | 3211 | 24.18x |
| planExtractByPathHex: 1000 of 4000 | 83163 | 4352 | 19.11x |
| readEntry: 1000 of 4000 | 940 | 909 | no change |
| extractAll: tes3, 1000 entries, over the last run | 159466 | 74666 | 2.14x |
| extractAll: tes3, 1000 entries, into a new folder, then removed | 158719 | 74427 | 2.13x |
| extractMany: 250 of 1000, over the last run | 39618 | 19594 | 2.02x |
| planExtractAll: tes3, 1000 entries, over files that are there | 78183 | 5679 | 13.77x |
| planCreate: tes3, a folder of 1000 files | 53449 | 41906 | 1.28x |
| create: tes3, a folder of 1000 files | 56136 | 49340 | 1.14x |
| planAdd: 250 files to tes3, 1000 entries | 27029 | 18205 | 1.48x |
| add: 250 files to tes3, 1000 entries | 21722 | 21969 | no change |
| command line: list | 30829 | 4315 | 7.14x |
| command line: list --long | 32271 | 5543 | 5.82x |
| command line: list --json | 53508 | 13344 | 4.01x |
| command line: verify --json | 8819 | 4554 | 1.94x |
| command line: diff --hash --json | 44789 | 28536 | 1.57x |
| command line: extract-all --dry-run | 384273 | 26890 | 14.29x |
| command line: parse 8 command lines | 19.4 | 20.4 | no change |

`create`'s three base runs ran from 47.9 to 56.2 ms and its head runs from 49.3 to 50.0 ms, so
its 1.14x is inside base's own spread: call it no change. `parse` is 19.4 against 20.4 µs here
and 20.6 against 21.3 µs in seven more alternated runs of that row alone, spreads overlapping:
no change.

## What moved it

None of it is the dialect. The comprehensions, reducers, slice, and record patterns the package
now uses lower to the loops they replaced, and the commits that made them, run against the
commit before them (`a2e8e37` against `1161844`, the whole bench, alternated three times), left
every row within the spread of its own runs. What moved the rest was Luau doing by hand what a
native call or a table does. Each commit below was run against the one before it, alternated
under the lock, three rounds for the first four and five for the rest; the numbers are medians,
in milliseconds.

- **A repeated extraction target was found by sorting** (`04982e8`). Every plan and extraction
  sorted its targets with a comparison that split both paths into components, each time. Two
  targets are one path exactly when their components joined are one string, so a table finds a
  repeat, and the sort now runs only to name one in the error. `planExtractAll` 337 to 50.9
  (6.6x), `planExtract` 72.1 to 13.3 (5.4x), `extract-all --dry-run` 380 to 88.1 (4.3x),
  `extractAll` 147.5 to 89.4 (1.65x), `extractMany` 37.1 to 25.7 (1.45x).
- **A path's hex was a callback for each byte** (`facb282`). `bytes.toHex` writes it in one
  call, and `bytes.fromHex` reads it. `list` 29.8 to 3.7 (8.1x), the BA2 `list` 7.8 to 1.2
  (6.3x), `diff` by size 15.6 to 8.4 (1.86x), the plans about 2x again (`planExtractAll` 50.9
  to 24.3), `planAdd` 24.8 to 17.3 (1.43x), `planCreate` 48.9 to 40.9 (1.20x), `list --json`
  51.7 to 26.3 (1.97x).
- **A path's folder and file name were found with patterns** that walk the whole path
  (`c211b3a`). `bytes.rfind` finds the last separator, and the patterns stay for the paths that
  need them. `extractAll` 90.7 to 74.5 (1.22x), `extractMany` 26.0 to 22.3 (1.17x), and nothing
  else: `planCreate` is 1.04x, which is no change.
- **Every path was split to decide whether it could be extracted, and again to put it below
  the output** (`44c2f98`). A path with no `:`, no two dots together, and something besides
  dots is extractable without splitting, and a key with no `.` component to drop goes below the
  output as it is. `verify` 8.4 to 3.9 (2.15x), `verify --json` 8.7 to 4.4, the plans 1.4x to
  1.55x (`planExtractAll` 24.8 to 17.4).
- **The folders an extraction makes were split into components in every comparison of the sort
  that orders them** (`bff8ca1`). Split once, before sorting: `extractMany` 23.1 to 20.0
  (1.16x), `extractAll` over the last run 79.4 to 75.1 (1.06x), and nothing `extractAll` into a
  new folder can show.
- **JSON was quoted by `string.gsub` over every string**, field names included, for every row.
  One compiled `bytes.regex` now says whether a string needs an escape at all (`6599eb0`:
  `list --json` 27.7 to 17.9, 1.55x; `diff --hash --json` 34.5 to 30.6, 1.13x), and a field name
  is quoted once (`a2e8e37`: `list --json` 17.9 to 14.7, 1.22x; `diff --hash --json` 1.09x). An
  object also keeps the pairs it is given instead of making a record of each (`f330219`:
  `list --json` 13.8 to 12.3, 1.12x; `diff --hash --json` 1.07x; `extract-all --dry-run` 1.06x).
- **A plan asked the disk about every target** (`e86cc19`). Nothing is below a folder that
  isn't there, so a plan and the check before an extraction now ask about each folder once, and
  look for a file only in a folder that is there. Into an output that isn't there yet:
  `planExtractAll` 17.7 to 11.0 (1.61x), `planExtract` 4.5 to 3.1 (1.44x),
  `extract-all --dry-run` 33.6 to 24.8 (1.36x). It costs something where everything is there:
  `planExtractAll ... over files that are there` went from 5.07 to 5.51, 8 percent slower, one
  more look for each folder and a table read for each target. Whether something is there was
  compared with asking about the path itself over 140,896 paths on Linux (folders, files where
  a folder would be, links to both, a link to nothing, a folder that can't be read, `.` and
  `..`), in three orders; it was not run on Windows or macOS.

What did not move is what the disk and `@dream-archive` do: `readEntry` is a lookup and a read,
`info` counts names, and `create` and `add` spend their time resolving each input to refuse a
symbolic link, reading the files, and writing the archive.

## What the dialect costs a run that starts cold

The command line loads its modules once and exits, and every module's first comprehension,
reducer, or slice is lowered when it loads. Loading `@dream-archivetool/cli` from source, timed
inside 25 fresh processes a side, alternated, median and fastest in milliseconds:

| Package at | Load, median | Load, fastest |
|---|---:|---:|
| `73e4749`, base | 92.5 | 90.2 |
| `a2e8e37`, every change for speed but the last two, one comprehension | 93.8 | 91.5 |
| `1161844`, the dialect conversions on top of it | 97.2 | 94.1 |
| head | 96.6 | 93.7 |

The conversions add about 3.5 ms to a load of 93, under the 5 percent this machine can tell
apart in one run and there in every run. A whole `list --json` of a 20-entry archive, process
and all, took 125.1 ms at base and 127.3 ms at head (median of five alternated rounds of 40
runs, head the slower in each). An archive of a few hundred entries repays it: the command line
lists 4,000 entries 26 ms sooner.

## Rows that are loops again

`render` is handed a result its caller may have built, so the two places it walks a result's
rows (`list`, and every JSON array) are the loops they were, not comprehensions: a comprehension
raised on a list with a gap, or on a row without a path, where the loop printed what it found.
Putting them back cost nothing: `list --json` 14.2 ms before and 13.2 ms after, `list --long`
5.7 and 5.4, `list` 4.5 and 4.6, alternated five times, spreads overlapping.
