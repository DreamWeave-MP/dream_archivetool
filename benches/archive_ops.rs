use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use dream_archivetool::{
    AddOptions, ArchiveFormat, ArchiveTool, CreateOptions, DiffOptions, ExtractAllOptions,
    ExtractOptions, OverwriteMode, VerifyOptions,
};
use tempfile::TempDir;

struct TrackingAllocator;

static CURRENT_ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static PEAK_ALLOCATED: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

// SAFETY: This allocator delegates all allocation behavior to `System` and only records byte counts
// using the `Layout` sizes supplied by the allocator API.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: Delegating to the system allocator with the layout supplied by the caller.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            add_allocated(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: Delegating to the system allocator with the same pointer/layout contract.
        unsafe { System.dealloc(ptr, layout) };
        CURRENT_ALLOCATED.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: Delegating to the system allocator with the pointer/layout/new-size contract.
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            let old_size = layout.size();
            if new_size >= old_size {
                add_allocated(new_size - old_size);
            } else {
                CURRENT_ALLOCATED.fetch_sub(old_size - new_size, Ordering::Relaxed);
            }
        }
        new_ptr
    }
}

fn add_allocated(size: usize) {
    let current = CURRENT_ALLOCATED.fetch_add(size, Ordering::Relaxed) + size;
    let mut peak = PEAK_ALLOCATED.load(Ordering::Relaxed);
    while current > peak {
        match PEAK_ALLOCATED.compare_exchange_weak(
            peak,
            current,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(observed) => peak = observed,
        }
    }
}

fn reset_peak() {
    let current = CURRENT_ALLOCATED.load(Ordering::Relaxed);
    PEAK_ALLOCATED.store(current, Ordering::Relaxed);
}

fn peak_delta() -> usize {
    PEAK_ALLOCATED
        .load(Ordering::Relaxed)
        .saturating_sub(CURRENT_ALLOCATED.load(Ordering::Relaxed))
}

fn measure_peak_bytes<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    reset_peak();
    let value = operation();
    (value, peak_delta())
}

fn report_peak(label: &str, bytes: usize) {
    eprintln!("peak allocator delta for {label}: {}", format_bytes(bytes));
}

fn format_bytes(bytes: usize) -> String {
    const KIB: usize = 1024;
    const MIB: usize = KIB * 1024;
    if bytes >= MIB {
        format_fixed_unit(bytes, MIB, "MiB")
    } else if bytes >= KIB {
        format_fixed_unit(bytes, KIB, "KiB")
    } else {
        format!("{bytes} B")
    }
}

fn format_fixed_unit(bytes: usize, unit: usize, suffix: &str) -> String {
    let whole = bytes / unit;
    let hundredths = bytes % unit * 100 / unit;
    format!("{whole}.{hundredths:02} {suffix}")
}

struct ArchiveFixture {
    _dir: TempDir,
    archive: PathBuf,
    entries: Vec<Vec<u8>>,
}

fn write_input_tree(root: &Path, entry_count: usize, payload_len: usize, prefix: &str) {
    let payload = vec![b'x'; payload_len];
    for index in 0..entry_count {
        let dir = root.join(format!("{prefix}_{:02}", index % 16));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("entry_{index:05}.bin")), &payload).unwrap();
    }
}

fn fixture_archive(entry_count: usize, payload_len: usize) -> ArchiveFixture {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input");
    write_input_tree(&input, entry_count, payload_len, "group");
    let archive = dir.path().join("fixture.bsa");
    ArchiveTool::create(
        &archive,
        &input,
        &CreateOptions {
            format: ArchiveFormat::Tes3,
            ..CreateOptions::default()
        },
    )
    .unwrap();
    let entries = ArchiveTool::list(&archive)
        .unwrap()
        .into_iter()
        .map(|entry| dream_archivetool::decode_archive_path_hex(&entry.path_bytes_hex).unwrap())
        .collect();
    ArchiveFixture {
        _dir: dir,
        archive,
        entries,
    }
}

fn fixture_archive_with_format(
    format: ArchiveFormat,
    entry_count: usize,
    payload_len: usize,
    extension: &str,
) -> ArchiveFixture {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input");
    let data = input.join("data");
    fs::create_dir_all(&data).unwrap();
    let payload = vec![b'f'; payload_len];
    for index in 0..entry_count {
        fs::write(data.join(format!("entry_{index:05}.{extension}")), &payload).unwrap();
    }
    let archive = dir.path().join(match format {
        ArchiveFormat::Ba2 => "fixture.ba2",
        ArchiveFormat::Tes3 | ArchiveFormat::Tes4 => "fixture.bsa",
    });
    ArchiveTool::create(
        &archive,
        &input,
        &CreateOptions {
            format,
            ..CreateOptions::default()
        },
    )
    .unwrap();
    let entries = ArchiveTool::list(&archive)
        .unwrap()
        .into_iter()
        .map(|entry| dream_archivetool::decode_archive_path_hex(&entry.path_bytes_hex).unwrap())
        .collect();
    ArchiveFixture {
        _dir: dir,
        archive,
        entries,
    }
}

fn extract_options(output: &Path, overwrite: OverwriteMode) -> ExtractOptions {
    ExtractOptions {
        output: Some(output.to_path_buf()),
        overwrite,
        preserve_paths: true,
        fsync: false,
    }
}

fn bench_read_only(c: &mut Criterion) {
    const ENTRY_COUNT: usize = 256;
    const PAYLOAD_LEN: usize = 4096;
    let fixture = fixture_archive(ENTRY_COUNT, PAYLOAD_LEN);
    let selected = fixture.entries[ENTRY_COUNT / 2].clone();

    let (_, peak) = measure_peak_bytes(|| ArchiveTool::list(&fixture.archive).unwrap());
    report_peak("list generated archive", peak);

    let mut group = c.benchmark_group("read_only");
    group.bench_function(BenchmarkId::new("list", ENTRY_COUNT), |b| {
        b.iter(|| black_box(ArchiveTool::list(black_box(&fixture.archive)).unwrap()));
    });

    let (_, peak) = measure_peak_bytes(|| {
        let archive = ArchiveTool::open(&fixture.archive).unwrap();
        archive.read_entry_by_path_bytes(&selected).unwrap()
    });
    report_peak("single entry read from opened archive", peak);

    group.bench_function(BenchmarkId::new("open_and_read_entry", PAYLOAD_LEN), |b| {
        b.iter(|| {
            let archive = ArchiveTool::open(black_box(&fixture.archive)).unwrap();
            black_box(
                archive
                    .read_entry_by_path_bytes(black_box(&selected))
                    .unwrap(),
            );
        });
    });

    let (_, peak) = measure_peak_bytes(|| {
        ArchiveTool::verify(
            &fixture.archive,
            &VerifyOptions {
                read_payloads: true,
            },
        )
        .unwrap()
    });
    report_peak("verify with payload streaming", peak);

    group.bench_function(BenchmarkId::new("verify_read_payloads", ENTRY_COUNT), |b| {
        b.iter(|| {
            black_box(
                ArchiveTool::verify(
                    black_box(&fixture.archive),
                    &VerifyOptions {
                        read_payloads: true,
                    },
                )
                .unwrap(),
            );
        });
    });
    group.finish();
}

fn bench_extract(c: &mut Criterion) {
    const ENTRY_COUNT: usize = 192;
    const PAYLOAD_LEN: usize = 4096;
    let fixture = fixture_archive(ENTRY_COUNT, PAYLOAD_LEN);
    let mut group = c.benchmark_group("extract");

    bench_extract_all(&mut group, &fixture, ENTRY_COUNT);
    bench_extract_skip_existing(&mut group, &fixture, ENTRY_COUNT);
    bench_extract_many(&mut group, &fixture);

    group.finish();
}

fn bench_extract_all(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    fixture: &ArchiveFixture,
    entry_count: usize,
) {
    let (_, peak) = measure_peak_bytes(|| {
        let out = TempDir::new().unwrap();
        ArchiveTool::extract_all(
            &fixture.archive,
            &ExtractAllOptions {
                output: Some(out.path().to_path_buf()),
                overwrite: OverwriteMode::Fail,
                fsync: false,
            },
        )
        .unwrap()
    });
    report_peak("extract-all generated archive", peak);

    group.bench_function(BenchmarkId::new("extract_all", entry_count), |b| {
        b.iter_batched(
            || TempDir::new().unwrap(),
            |out| {
                black_box(
                    ArchiveTool::extract_all(
                        black_box(&fixture.archive),
                        &ExtractAllOptions {
                            output: Some(out.path().to_path_buf()),
                            overwrite: OverwriteMode::Fail,
                            fsync: false,
                        },
                    )
                    .unwrap(),
                );
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_extract_skip_existing(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    fixture: &ArchiveFixture,
    entry_count: usize,
) {
    let skip_out = TempDir::new().unwrap();
    ArchiveTool::extract_all(
        &fixture.archive,
        &ExtractAllOptions {
            output: Some(skip_out.path().to_path_buf()),
            overwrite: OverwriteMode::Fail,
            fsync: false,
        },
    )
    .unwrap();

    let (_, peak) = measure_peak_bytes(|| {
        ArchiveTool::extract_all(
            &fixture.archive,
            &ExtractAllOptions {
                output: Some(skip_out.path().to_path_buf()),
                overwrite: OverwriteMode::Skip,
                fsync: false,
            },
        )
        .unwrap()
    });
    report_peak("extract-all skip-existing", peak);

    group.bench_function(
        BenchmarkId::new("extract_all_skip_existing", entry_count),
        |b| {
            b.iter(|| {
                black_box(
                    ArchiveTool::extract_all(
                        black_box(&fixture.archive),
                        &ExtractAllOptions {
                            output: Some(skip_out.path().to_path_buf()),
                            overwrite: OverwriteMode::Skip,
                            fsync: false,
                        },
                    )
                    .unwrap(),
                );
            });
        },
    );
}

fn bench_extract_many(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    fixture: &ArchiveFixture,
) {
    let selected: Vec<Vec<u8>> = fixture.entries.iter().take(32).cloned().collect();
    let (_, peak) = measure_peak_bytes(|| {
        let out = TempDir::new().unwrap();
        ArchiveTool::extract_many_by_path_bytes(
            &fixture.archive,
            &selected,
            &extract_options(out.path(), OverwriteMode::Fail),
        )
        .unwrap()
    });
    report_peak("selected batch extraction", peak);

    group.bench_function(BenchmarkId::new("extract_many", selected.len()), |b| {
        b.iter_batched(
            || TempDir::new().unwrap(),
            |out| {
                black_box(
                    ArchiveTool::extract_many_by_path_bytes(
                        black_box(&fixture.archive),
                        black_box(&selected),
                        &extract_options(out.path(), OverwriteMode::Fail),
                    )
                    .unwrap(),
                );
            },
            BatchSize::SmallInput,
        );
    });
}

fn create_tes3(output: impl AsRef<Path>, input: impl AsRef<Path>) -> usize {
    ArchiveTool::create(
        output,
        input,
        &CreateOptions {
            format: ArchiveFormat::Tes3,
            ..CreateOptions::default()
        },
    )
    .unwrap()
}

fn create_input_case(entry_count: usize, payload_len: usize, prefix: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input");
    write_input_tree(&input, entry_count, payload_len, prefix);
    (dir, input)
}

fn add_preserve_case(entry_count: usize, payload_len: usize) -> (TempDir, PathBuf, PathBuf) {
    let dir = TempDir::new().unwrap();
    let base_input = dir.path().join("base");
    write_input_tree(&base_input, entry_count, payload_len, "base");
    let archive = dir.path().join("base.bsa");
    create_tes3(&archive, &base_input);
    let new_file = dir.path().join("new_entry.bin");
    fs::write(&new_file, vec![b'n'; payload_len]).unwrap();
    (dir, archive, new_file)
}

fn add_preserving_entries(dir: &Path, archive: &Path, new_file: PathBuf) -> usize {
    ArchiveTool::add(
        archive,
        &AddOptions {
            inputs: vec![new_file],
            output: Some(dir.join("updated.bsa")),
            fsync: false,
            follow_symlinks: false,
        },
    )
    .unwrap()
}

fn bench_format_smoke(c: &mut Criterion) {
    const ENTRY_COUNT: usize = 96;
    const PAYLOAD_LEN: usize = 2048;
    let tes4 = fixture_archive_with_format(ArchiveFormat::Tes4, ENTRY_COUNT, PAYLOAD_LEN, "dds");
    let ba2 = fixture_archive_with_format(ArchiveFormat::Ba2, ENTRY_COUNT, PAYLOAD_LEN, "txt");
    let mut group = c.benchmark_group("format_smoke");

    bench_format_list(&mut group, "tes4", &tes4, ENTRY_COUNT);
    bench_format_list(&mut group, "ba2", &ba2, ENTRY_COUNT);
    bench_format_extract_all(&mut group, "tes4", &tes4, ENTRY_COUNT);
    bench_format_extract_all(&mut group, "ba2", &ba2, ENTRY_COUNT);

    group.finish();
}

fn bench_format_list(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    name: &str,
    fixture: &ArchiveFixture,
    entry_count: usize,
) {
    let (_, peak) = measure_peak_bytes(|| ArchiveTool::list(&fixture.archive).unwrap());
    report_peak(&format!("list {name} archive"), peak);
    group.bench_function(BenchmarkId::new(format!("list_{name}"), entry_count), |b| {
        b.iter(|| black_box(ArchiveTool::list(black_box(&fixture.archive)).unwrap()));
    });
}

fn bench_format_extract_all(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    name: &str,
    fixture: &ArchiveFixture,
    entry_count: usize,
) {
    let (_, peak) = measure_peak_bytes(|| {
        let out = TempDir::new().unwrap();
        ArchiveTool::extract_all(
            &fixture.archive,
            &ExtractAllOptions {
                output: Some(out.path().to_path_buf()),
                overwrite: OverwriteMode::Fail,
                fsync: false,
            },
        )
        .unwrap()
    });
    report_peak(&format!("extract-all {name} archive"), peak);
    group.bench_function(
        BenchmarkId::new(format!("extract_all_{name}"), entry_count),
        |b| {
            b.iter_batched(
                || TempDir::new().unwrap(),
                |out| {
                    black_box(
                        ArchiveTool::extract_all(
                            black_box(&fixture.archive),
                            &ExtractAllOptions {
                                output: Some(out.path().to_path_buf()),
                                overwrite: OverwriteMode::Fail,
                                fsync: false,
                            },
                        )
                        .unwrap(),
                    );
                },
                BatchSize::SmallInput,
            );
        },
    );
}

fn bench_create_and_update(c: &mut Criterion) {
    const ENTRY_COUNT: usize = 128;
    const PAYLOAD_LEN: usize = 4096;
    let mut group = c.benchmark_group("create_update");

    bench_create_tes3(&mut group, ENTRY_COUNT, PAYLOAD_LEN);
    bench_add_preserve_rewrite(&mut group, ENTRY_COUNT, PAYLOAD_LEN);
    bench_diff_payload_fingerprint(&mut group, ENTRY_COUNT, PAYLOAD_LEN);

    group.finish();
}

fn bench_create_tes3(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    entry_count: usize,
    payload_len: usize,
) {
    let (_, peak) = measure_peak_bytes(|| {
        let (dir, input) = create_input_case(entry_count, payload_len, "create");
        create_tes3(dir.path().join("created.bsa"), &input)
    });
    report_peak("create generated archive", peak);

    group.bench_function(BenchmarkId::new("create_tes3", entry_count), |b| {
        b.iter_batched(
            || create_input_case(entry_count, payload_len, "create"),
            |(dir, input)| {
                black_box(create_tes3(
                    dir.path().join("created.bsa"),
                    black_box(&input),
                ));
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_add_preserve_rewrite(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    entry_count: usize,
    payload_len: usize,
) {
    let (_, peak) = measure_peak_bytes(|| {
        let (dir, archive, new_file) = add_preserve_case(entry_count, payload_len);
        add_preserving_entries(dir.path(), &archive, new_file)
    });
    report_peak("add preserving unchanged entries", peak);

    group.bench_function(BenchmarkId::new("add_preserve_rewrite", entry_count), |b| {
        b.iter_batched(
            || add_preserve_case(entry_count, payload_len),
            |(dir, archive, new_file)| {
                black_box(add_preserving_entries(
                    dir.path(),
                    black_box(&archive),
                    new_file,
                ));
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_diff_payload_fingerprint(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    entry_count: usize,
    payload_len: usize,
) {
    let old_fixture = fixture_archive(entry_count, payload_len);
    let new_fixture = fixture_archive(entry_count, payload_len + 1);
    let (_, peak) = measure_peak_bytes(|| {
        ArchiveTool::diff(
            &old_fixture.archive,
            &new_fixture.archive,
            &DiffOptions {
                fingerprint_payloads: true,
            },
        )
        .unwrap()
    });
    report_peak("diff with payload fingerprints", peak);

    group.bench_function(
        BenchmarkId::new("diff_payload_fingerprint", entry_count),
        |b| {
            b.iter(|| {
                black_box(
                    ArchiveTool::diff(
                        black_box(&old_fixture.archive),
                        black_box(&new_fixture.archive),
                        &DiffOptions {
                            fingerprint_payloads: true,
                        },
                    )
                    .unwrap(),
                );
            });
        },
    );
}

criterion_group!(
    benches,
    bench_read_only,
    bench_extract,
    bench_format_smoke,
    bench_create_and_update
);
criterion_main!(benches);
