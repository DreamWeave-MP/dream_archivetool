// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Luau boundary of the policy layer: plans, reports, and batch extraction from scripts.
//!
//! The scripts are frozen at the pre-migration baseline so the numbers stay comparable across
//! the mlua-to-l3i migration; only the harness that runs them changes.

#![allow(clippy::missing_panics_doc, clippy::semicolon_if_nothing_returned)]

use std::path::Path;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use dream_archivetool::luau::ArchivetoolExtension;
use l3i::Runtime;
use l3i::extension::{RuntimePlan, RuntimePolicy};
use tempfile::TempDir;

const PLAN_ENTRIES: usize = 2000;
const EXTRACT_ENTRIES: usize = 256;

/// One script per scenario. `archive` and `other` are opened handles (`other` differs in one
/// payload and one added member), `paths` the archive's member paths in listing order,
/// `few` the first 256 of them, `out` a scratch output directory.
const SCRIPTS: &[(&str, &str)] = &[
    (
        "plan_extract_2000",
        "local plan = archive:planExtract(paths, { output = out }) return #plan.entries",
    ),
    (
        "plan_extract_all_iterate",
        "local n = 0 for _, row in archive:planExtractAll({ output = out }).entries do \
         if row.action == 'extract' and #row.pathBytesHex > 0 then n += 1 end end return n",
    ),
    (
        "plan_extract_all_to_table",
        "local rows = archive:planExtractAll({ output = out }).entries \
         local n = 0 for i = 1, #rows do if rows[i].target ~= '' then n += 1 end end return n",
    ),
    (
        "extract_many_256",
        "return archive:extractMany(few, { output = out, overwrite = 'overwrite' }).extracted",
    ),
    (
        "verify_report",
        "local report = archive:verify() return report.fileCount + #report.warnings + #report.unsafePaths",
    ),
    (
        "diff_report",
        "local report = archive:diff(other, { fingerprintPayloads = true }) \
         return #report.added + #report.removed + #report.changed + report.unchanged",
    ),
    (
        "diff_iterate",
        "local report = archive:diff(other, {}) local n = 0 \
         for _, entry in report.added do n += #entry.path end \
         for _, change in report.changed do n += #change.path end return n",
    ),
    (
        "tool_info",
        "local info = archive:toolInfo() return info.fileCount",
    ),
];

fn entry_path(index: usize) -> String {
    format!("meshes/folder{:03}/file{index:05}.nif", index % 128)
}

fn write_archive(path: &Path, changed: bool) {
    let mut builder = dream_archive::Tes3BsaBuilder::new();
    for index in 0..PLAN_ENTRIES {
        let payload = if changed && index == 7 {
            b"changed payload".to_vec()
        } else {
            vec![u8::try_from(index % 251).unwrap(); 16]
        };
        builder.add_bytes(entry_path(index), payload).unwrap();
    }
    if changed {
        builder.add_bytes("meshes/added.nif", b"added").unwrap();
    }
    builder.write_path(path).unwrap();
}

/// The chunk that binds the scenario's locals and returns the scenario as a function.
fn chunk(body: &str) -> String {
    format!(
        "local archive = dreamArchive.openPath(archivePath) \
         local other = dreamArchive.openPath(otherPath) \
         local paths, few = {{}}, {{}} \
         for _, e in archive:entries() do table.insert(paths, e.path) if #few < {EXTRACT_ENTRIES} then table.insert(few, e.path) end end \
         return function() {body} end"
    )
}

fn luau_boundary(c: &mut Criterion) {
    let dir = TempDir::new().unwrap();
    let archive = dir.path().join("fixture.bsa");
    let other = dir.path().join("other.bsa");
    let out = dir.path().join("out");
    write_archive(&archive, false);
    write_archive(&other, true);

    let plan = RuntimePlan::builder()
        .policy(
            RuntimePolicy::new()
                .compat_global("@dream/archive", "dreamArchive")
                .compat_global("@dream/archivetool", "dreamArchivetool"),
        )
        .extension(dream_archive::luau::ArchiveExtension)
        .extension(ArchivetoolExtension)
        .finalize()
        .unwrap();
    let runtime = Runtime::from_plan(&plan).unwrap();
    for (name, value) in [
        ("archivePath", &archive),
        ("otherPath", &other),
        ("out", &out),
    ] {
        runtime
            .set_global(name, value.to_string_lossy().as_ref())
            .unwrap();
    }
    let mut group = c.benchmark_group("archivetool");
    for (name, body) in SCRIPTS {
        let function = runtime.load_function(&chunk(body)).unwrap();
        let stack = runtime.stack();
        group.bench_function(*name, |b| {
            b.iter(|| function.invoke::<f64, _>(&stack, ()).unwrap())
        });
    }
    group.finish();
}

fn configure() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2))
        .sample_size(20)
}

criterion_group! { name = benches; config = configure(); targets = luau_boundary }
criterion_main!(benches);
