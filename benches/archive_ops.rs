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

fn measure_peak_bytes<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    let baseline = CURRENT_ALLOCATED.load(Ordering::Relaxed);
    PEAK_ALLOCATED.store(baseline, Ordering::Relaxed);
    let value = operation();
    let peak = PEAK_ALLOCATED
        .load(Ordering::Relaxed)
        .saturating_sub(baseline);
    (value, peak)
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
        _ => "fixture.archive",
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

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_zeros(out: &mut Vec<u8>, count: usize) {
    out.resize(out.len() + count, 0);
}

fn dx10_dds(width: u32, height: u32, payload: &[u8]) -> Vec<u8> {
    const DDSD_CAPS: u32 = 0x0000_0001;
    const DDSD_HEIGHT: u32 = 0x0000_0002;
    const DDSD_WIDTH: u32 = 0x0000_0004;
    const DDSD_PIXELFORMAT: u32 = 0x0000_1000;
    const DDSD_MIPMAPCOUNT: u32 = 0x0002_0000;
    const DDSD_LINEARSIZE: u32 = 0x0008_0000;
    const DDPF_FOURCC: u32 = 0x0000_0004;
    const DDSCAPS_TEXTURE: u32 = 0x0000_1000;
    const DDS_DIMENSION_TEXTURE2D: u32 = 3;
    const DXGI_FORMAT_BC7_UNORM: u32 = 98;

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"DDS ");
    push_u32(&mut bytes, 124);
    push_u32(
        &mut bytes,
        DDSD_CAPS
            | DDSD_HEIGHT
            | DDSD_WIDTH
            | DDSD_PIXELFORMAT
            | DDSD_MIPMAPCOUNT
            | DDSD_LINEARSIZE,
    );
    push_u32(&mut bytes, height);
    push_u32(&mut bytes, width);
    push_u32(&mut bytes, payload.len().try_into().unwrap());
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 1);
    push_zeros(&mut bytes, 44);
    push_u32(&mut bytes, 32);
    push_u32(&mut bytes, DDPF_FOURCC);
    push_u32(&mut bytes, u32::from_le_bytes(*b"DX10"));
    push_zeros(&mut bytes, 20);
    push_u32(&mut bytes, DDSCAPS_TEXTURE);
    push_zeros(&mut bytes, 16);
    push_u32(&mut bytes, DXGI_FORMAT_BC7_UNORM);
    push_u32(&mut bytes, DDS_DIMENSION_TEXTURE2D);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 0);
    bytes.extend_from_slice(payload);
    bytes
}

fn dx10_add_preserve_case(
    entry_count: usize,
    dds_payload_len: usize,
) -> (TempDir, PathBuf, PathBuf) {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("dx10-base");
    let textures = input.join("textures");
    fs::create_dir_all(&textures).unwrap();
    let payload = vec![0xabu8; dds_payload_len];
    for index in 0..entry_count {
        fs::write(
            textures.join(format!("preserved_{index:03}.dds")),
            dx10_dds(1024, 1024, &payload),
        )
        .unwrap();
    }
    let archive = dir.path().join("dx10.ba2");
    ArchiveTool::create(
        &archive,
        &input,
        &CreateOptions {
            format: ArchiveFormat::Ba2,
            ba2_kind: dream_archivetool::Ba2ArchiveKind::Dx10,
            ..CreateOptions::default()
        },
    )
    .unwrap();
    let replacement = dir.path().join("replacement.dds");
    fs::write(&replacement, dx10_dds(1024, 1024, &payload)).unwrap();
    (dir, archive, replacement)
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

    let opened_for_peak = ArchiveTool::open(&fixture.archive).unwrap();
    let (_, peak) =
        measure_peak_bytes(|| opened_for_peak.read_entry_by_path_bytes(&selected).unwrap());
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

    let opened = ArchiveTool::open(&fixture.archive).unwrap();
    group.bench_function(BenchmarkId::new("opened_read_entry", PAYLOAD_LEN), |b| {
        b.iter(|| {
            black_box(
                opened
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
    bench_ba2_dx10_add_preserve_rewrite(&mut group);
    bench_diff_payload_fingerprint(&mut group, ENTRY_COUNT, PAYLOAD_LEN);

    group.finish();
}

fn bench_create_tes3(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    entry_count: usize,
    payload_len: usize,
) {
    let (peak_dir, peak_input) = create_input_case(entry_count, payload_len, "create-peak");
    let (_, peak) =
        measure_peak_bytes(|| create_tes3(peak_dir.path().join("created.bsa"), &peak_input));
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
    let (peak_dir, peak_archive, peak_new_file) = add_preserve_case(entry_count, payload_len);
    let (_, peak) = measure_peak_bytes(|| {
        add_preserving_entries(peak_dir.path(), &peak_archive, peak_new_file)
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

fn bench_ba2_dx10_add_preserve_rewrite(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
) {
    const ENTRY_COUNT: usize = 4;
    const DDS_PAYLOAD_LEN: usize = 1024 * 1024;
    let (peak_dir, peak_archive, peak_new_file) =
        dx10_add_preserve_case(ENTRY_COUNT, DDS_PAYLOAD_LEN);
    let (_, peak) = measure_peak_bytes(|| {
        add_preserving_entries(peak_dir.path(), &peak_archive, peak_new_file)
    });
    report_peak("BA2 DX10 add preserving buffered DDS entries", peak);

    group.bench_function(
        BenchmarkId::new("ba2_dx10_add_preserve_buffering", ENTRY_COUNT),
        |b| {
            b.iter_batched(
                || dx10_add_preserve_case(ENTRY_COUNT, DDS_PAYLOAD_LEN),
                |(dir, archive, new_file)| {
                    black_box(add_preserving_entries(
                        dir.path(),
                        black_box(&archive),
                        new_file,
                    ));
                },
                BatchSize::SmallInput,
            );
        },
    );
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

fn bench_large_payload_streaming(c: &mut Criterion) {
    const ENTRY_COUNT: usize = 4;
    const PAYLOAD_LEN: usize = 8 * 1024 * 1024;
    let fixture = fixture_archive(ENTRY_COUNT, PAYLOAD_LEN);
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

    let mut group = c.benchmark_group("large_payload_streaming");
    bench_large_extract_all(&mut group, &fixture, ENTRY_COUNT, PAYLOAD_LEN);
    bench_large_verify(&mut group, &fixture, ENTRY_COUNT, PAYLOAD_LEN);
    bench_large_skip_existing(&mut group, &fixture, skip_out.path(), ENTRY_COUNT);
    group.finish();
}

fn bench_large_extract_all(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    fixture: &ArchiveFixture,
    entry_count: usize,
    payload_len: usize,
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
    report_peak("extract-all large payload streaming", peak);

    group.bench_function(
        BenchmarkId::new("extract_all", format!("{entry_count}x{payload_len}")),
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

fn bench_large_verify(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    fixture: &ArchiveFixture,
    entry_count: usize,
    payload_len: usize,
) {
    let (_, peak) = measure_peak_bytes(|| {
        ArchiveTool::verify(
            &fixture.archive,
            &VerifyOptions {
                read_payloads: true,
            },
        )
        .unwrap()
    });
    report_peak("verify large payload streaming", peak);

    group.bench_function(
        BenchmarkId::new(
            "verify_read_payloads",
            format!("{entry_count}x{payload_len}"),
        ),
        |b| {
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
        },
    );
}

fn bench_large_skip_existing(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    fixture: &ArchiveFixture,
    output: &Path,
    entry_count: usize,
) {
    let (_, peak) = measure_peak_bytes(|| {
        ArchiveTool::extract_all(
            &fixture.archive,
            &ExtractAllOptions {
                output: Some(output.to_path_buf()),
                overwrite: OverwriteMode::Skip,
                fsync: false,
            },
        )
        .unwrap()
    });
    report_peak("extract-all skip-existing large payload", peak);

    group.bench_function(BenchmarkId::new("skip_existing", entry_count), |b| {
        b.iter(|| {
            black_box(
                ArchiveTool::extract_all(
                    black_box(&fixture.archive),
                    &ExtractAllOptions {
                        output: Some(output.to_path_buf()),
                        overwrite: OverwriteMode::Skip,
                        fsync: false,
                    },
                )
                .unwrap(),
            );
        });
    });
}

fn bench_many_entries(c: &mut Criterion) {
    const ENTRY_COUNT: usize = 2_000;
    const PAYLOAD_LEN: usize = 128;
    let old_fixture = fixture_archive(ENTRY_COUNT, PAYLOAD_LEN);
    let new_fixture = fixture_archive(ENTRY_COUNT, PAYLOAD_LEN + 1);
    let mut group = c.benchmark_group("many_entries");

    bench_format_list(&mut group, "tes3_many", &old_fixture, ENTRY_COUNT);
    bench_diff_payload_many(&mut group, &old_fixture, &new_fixture, ENTRY_COUNT);
    bench_add_many_preserve(&mut group, ENTRY_COUNT, PAYLOAD_LEN);

    group.finish();
}

fn bench_diff_payload_many(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    old_fixture: &ArchiveFixture,
    new_fixture: &ArchiveFixture,
    entry_count: usize,
) {
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
    report_peak("diff many entries with payload fingerprints", peak);

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

fn bench_add_many_preserve(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    entry_count: usize,
    payload_len: usize,
) {
    let (peak_dir, peak_archive, peak_new_file) = add_preserve_case(entry_count, payload_len);
    let (_, peak) = measure_peak_bytes(|| {
        add_preserving_entries(peak_dir.path(), &peak_archive, peak_new_file)
    });
    report_peak("add preserving many unchanged entries", peak);

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

criterion_group!(
    benches,
    bench_read_only,
    bench_extract,
    bench_format_smoke,
    bench_create_and_update,
    bench_large_payload_streaming,
    bench_many_entries
);
criterion_main!(benches);
