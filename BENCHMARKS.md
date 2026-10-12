# Archive tool benchmarks

`bench/bench.luau` times the package through what it exports: the reports (`info`, `list`,
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
  jpk/dream-archivetool/bench/bench.luau <scratch folder> [filter]
```

## The move onto l3i 2.0, base against head

- Machine: Intel Core i7-10870H @ 2.20 GHz, 16 threads, shared with other agents' tests while
  this ran (load average 2.8 when it started, 5.3 when it ended).
- Date: 2026-10-12.
- Binary: the verified release build of Jess 0.1.0 on l3i 2.0.1 (Luau 0.742), the same one for
  both sides.
- Base: `73e4749`, the package before the move, running this bench as it is at head. Head:
  `a4a2812`.
- Protocol: the whole bench against base, then head, three times over, back to back under one
  lock; each cell is the median of its three runs. A difference under 5 percent is called no
  change.

| Workload | Base, µs | Head, µs | Base / head |
|---|---:|---:|---:|
| info: ba2, 1000 entries | 225 | 227 | no change |
| list: tes3, 4000 entries | 35159 | 4408 | 7.98x |
| list: ba2, 1000 entries | 8978 | 1402 | 6.40x |
| verify: tes3, 4000 entries | 9707 | 4420 | 2.20x |
| verify: tes3, 4000 entries, payloads read | 11400 | 6572 | 1.73x |
| diff: tes3, 4000 against 3800, by size | 17893 | 9829 | 1.82x |
| diff: tes3, 4000 against 3800, by fingerprint | 26445 | 16457 | 1.61x |
| planExtractAll: tes3, 4000 entries | 365207 | 12932 | 28.24x |
| planExtract: 1000 of 4000, by path | 79981 | 3728 | 21.45x |
| planExtractByPathHex: 1000 of 4000 | 84417 | 5162 | 16.35x |
| readEntry: 1000 of 4000 | 976 | 985 | no change |
| extractAll: tes3, 1000 entries, over the last run | 165202 | 80631 | 2.05x |
| extractAll: tes3, 1000 entries, into a new folder, then removed | 165987 | 82214 | 2.02x |
| extractMany: 250 of 1000, over the last run | 41611 | 22616 | 1.84x |
| planExtractAll: tes3, 1000 entries, over files that are there | 78270 | 7335 | 10.67x |
| planCreate: tes3, a folder of 1000 files | 59671 | 48448 | 1.23x |
| create: tes3, a folder of 1000 files | 60666 | 57614 | 1.05x |
| planAdd: 250 files to tes3, 1000 entries | 30478 | 20904 | 1.46x |
| add: 250 files to tes3, 1000 entries | 26001 | 25383 | no change |
| command line: list | 33410 | 4606 | 7.25x |
| command line: list --long | 35446 | 6239 | 5.68x |
| command line: list --json | 61200 | 14263 | 4.29x |
| command line: verify --json | 12689 | 4773 | 2.66x |
| command line: diff --hash --json | 62625 | 31249 | 2.00x |
| command line: extract-all --dry-run | 476419 | 26648 | 17.88x |
| command line: parse 8 command lines | 21.9 | 22.3 | no change |

Almost none of this is the dialect. The comprehensions, reducers, slice, and record patterns the
package now uses lower to the loops they replaced. The commits that made them (`a2e8e37` against
`3abeded`, alternated five times on the listing and diff rows) left every row within noise but
one: `list --json` went from 15.9 ms to 14.7 ms, 1.08x, where its rows and each row's fields
were lists built with `table.insert` and are now comprehensions. What moved the rest was Luau
doing by hand what a native call or a table does:

- **A repeated extraction target was found by sorting.** Every plan and extraction sorted its
  targets with a comparison that split both paths into components, each time. Two targets are
  one path exactly when their components joined are one string, so a table finds a repeat, and
  the sort now runs only to name one in the error. That is most of `planExtractAll` (328 ms to
  70 ms in a run of that commit alone), `planExtract`, and `extract-all --dry-run`, and a third
  of `extractAll`.
- **A path's hex was a callback for each byte.** `bytes.toHex` writes it in one call, and
  `bytes.fromHex` reads it. That is `list` (30 ms to 6 ms in a run of that commit), the rows of
  every plan and `diff`, and `planAdd`.
- **A path's folder and file name were found with patterns** that walk the whole path, which is
  slow on a long one: about 5 µs for the folder of a 150-byte target. `bytes.rfind` finds the
  last separator, and the patterns stay for the paths that need them. That is the rest of
  `extractAll` and `extractMany`, and what `planCreate` gained. Splitting the folders an
  extraction makes once, before sorting them, is worth about a tenth of `extractMany` on top
  (22.4 ms to 20.0 ms across two single runs), and nothing `extractAll` can show.
- **Every path was split to decide whether it could be extracted, and again to put it below
  the output.** A path with no `:`, no two dots together, and something besides dots is
  extractable without splitting, and a key with no `.` component to drop goes below the output
  as it is. That is `verify` (9.7 ms to 4.3 ms in a run of that commit) and another third off
  the plans.
- **JSON was quoted by `string.gsub` over every string**, field names included, for every row.
  One compiled `bytes.regex` now says whether a string needs an escape at all, and a field name
  is quoted once: `list --json` went from 27 ms to 14 ms across those two commits, in a run of
  each. An object also keeps the pairs it is given instead of making a record of each, which
  is 1.17x more on `list --json` (15.4 ms to 13.2 ms against the commit before it, alternated
  five times), 1.11x on `diff --hash --json`, and 1.07x on `extract-all --dry-run`.
- **A plan asked the disk about every target.** Nothing is below a folder that isn't there, so
  a plan and the check before an extraction now ask about each folder once, and look for a file
  only in a folder that is there. Into an output that isn't there yet, that halved
  `planExtractAll` again (19.8 ms to 10.6 ms in a run of that commit). It costs something
  where everything is there: `planExtractAll ... over files that are there` went from 5.5 ms to
  6.5 ms in the same pair of runs, one more look for each folder.

What did not move is what the disk and `@dream-archive` do: `readEntry` is a lookup and a read,
`info` counts names, and `create` and `add` spend their time resolving each input to refuse a
symbolic link, reading the files, and writing the archive. `create`'s 1.05x is at the edge of
what this machine can tell apart.
