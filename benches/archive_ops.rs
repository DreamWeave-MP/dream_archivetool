use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use dream_archivetool::{
    AddOptions, ArchiveFormat, ArchiveTool, CreateOptions, ExtractAllOptions, OverwriteMode,
};

fn unique_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dream-archivetool-bench-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn make_payload(index: usize, size: usize) -> Vec<u8> {
    (0..size)
        .map(|offset| u8::try_from(index.wrapping_add(offset) % 251).unwrap())
        .collect()
}

fn write_tes3_archive(path: &Path, entries: usize, payload_size: usize) {
    let mut builder = dream_archive::Tes3BsaBuilder::new();
    for index in 0..entries {
        let name = format!("textures/file-{index:05}.dds");
        builder
            .add_bytes(name.as_bytes(), make_payload(index, payload_size))
            .unwrap();
    }
    builder.write_path(path).unwrap();
}

fn write_input_tree(root: &Path, entries: usize, payload_size: usize) {
    let textures = root.join("textures");
    fs::create_dir_all(&textures).unwrap();
    for index in 0..entries {
        fs::write(
            textures.join(format!("file-{index:05}.dds")),
            make_payload(index, payload_size),
        )
        .unwrap();
    }
}

fn prepare_fixture(name: &str, entries: usize, payload_size: usize) -> (PathBuf, PathBuf) {
    let dir = unique_dir(name);
    fs::create_dir_all(&dir).unwrap();
    let archive = dir.join("fixture.bsa");
    write_tes3_archive(&archive, entries, payload_size);
    (dir, archive)
}

fn prepare_created_fixture(
    name: &str,
    format: ArchiveFormat,
    extension: &str,
    entries: usize,
    payload_size: usize,
) -> (PathBuf, PathBuf) {
    let dir = unique_dir(name);
    let input = dir.join("input");
    let data = input.join("data");
    fs::create_dir_all(&data).unwrap();
    for index in 0..entries {
        fs::write(
            data.join(format!("file-{index:05}.{extension}")),
            make_payload(index, payload_size),
        )
        .unwrap();
    }
    let archive = dir.join(match format {
        ArchiveFormat::Ba2 => "fixture.ba2",
        ArchiveFormat::Tes3 | ArchiveFormat::Tes4 => "fixture.bsa",
    });
    ArchiveTool::create(
        &archive,
        &input,
        &CreateOptions {
            format,
            ..Default::default()
        },
    )
    .unwrap();
    (dir, archive)
}

fn bench_list_and_lookup(c: &mut Criterion) {
    let (_dir, archive) = prepare_fixture("list-lookup", 1_000, 256);

    c.bench_function("list_tes3_1000_entries", |b| {
        b.iter(|| ArchiveTool::list(&archive).unwrap());
    });

    c.bench_function("read_entry_tes3_last_of_1000", |b| {
        b.iter(|| ArchiveTool::read_entry(&archive, "textures/file-00999.dds").unwrap());
    });

    c.bench_function("read_entry_tes3_missing_1000", |b| {
        b.iter(|| ArchiveTool::read_entry(&archive, "textures/missing.dds").unwrap_err());
    });

    c.bench_function("extract_entry_stdout_tes3_last_of_1000", |b| {
        b.iter(|| {
            let mut sink = io::sink();
            ArchiveTool::extract_entry_to_writer(&archive, "textures/file-00999.dds", &mut sink)
                .unwrap()
        });
    });

    c.bench_function("extract_entry_disk_tes3_last_of_1000", |b| {
        b.iter_batched(
            || {
                let dir = unique_dir("extract-one-output");
                fs::create_dir_all(&dir).unwrap();
                dir
            },
            |output| {
                let summary = ArchiveTool::extract(
                    &archive,
                    "TEXTURES//FILE-00999.DDS",
                    &dream_archivetool::ExtractOptions {
                        output: Some(output.clone()),
                        ..Default::default()
                    },
                )
                .unwrap();
                fs::remove_dir_all(output).unwrap();
                summary
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_extract_all(c: &mut Criterion) {
    let (_dir, archive) = prepare_fixture("extract", 512, 1_024);

    c.bench_function("extract_all_tes3_512x1k", |b| {
        b.iter_batched(
            || unique_dir("extract-output"),
            |output| {
                let summary = ArchiveTool::extract_all(
                    &archive,
                    &ExtractAllOptions {
                        output: Some(output.clone()),
                        overwrite: OverwriteMode::Fail,
                        fsync: false,
                    },
                )
                .unwrap();
                fs::remove_dir_all(output).unwrap();
                summary
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("extract_all_skip_existing_tes3_512x1k", |b| {
        b.iter_batched(
            || {
                let output = unique_dir("extract-skip-output");
                write_input_tree(&output, 512, 1_024);
                output
            },
            |output| {
                let summary = ArchiveTool::extract_all(
                    &archive,
                    &ExtractAllOptions {
                        output: Some(output.clone()),
                        overwrite: OverwriteMode::Skip,
                        fsync: false,
                    },
                )
                .unwrap();
                fs::remove_dir_all(output).unwrap();
                summary
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_tes4_and_ba2(c: &mut Criterion) {
    let (_tes4_dir, tes4) = prepare_created_fixture("tes4", ArchiveFormat::Tes4, "dds", 256, 512);
    let (_ba2_dir, ba2) = prepare_created_fixture("ba2", ArchiveFormat::Ba2, "txt", 256, 512);

    c.bench_function("list_tes4_256_entries", |b| {
        b.iter(|| ArchiveTool::list(&tes4).unwrap());
    });
    c.bench_function("read_entry_tes4_last_of_256", |b| {
        b.iter(|| ArchiveTool::read_entry(&tes4, "data/file-00255.dds").unwrap());
    });
    c.bench_function("extract_all_tes4_256x512", |b| {
        b.iter_batched(
            || unique_dir("extract-tes4-output"),
            |output| {
                let summary = ArchiveTool::extract_all(
                    &tes4,
                    &ExtractAllOptions {
                        output: Some(output.clone()),
                        overwrite: OverwriteMode::Fail,
                        fsync: false,
                    },
                )
                .unwrap();
                fs::remove_dir_all(output).unwrap();
                summary
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("list_ba2_256_entries", |b| {
        b.iter(|| ArchiveTool::list(&ba2).unwrap());
    });
    c.bench_function("read_entry_ba2_last_of_256", |b| {
        b.iter(|| ArchiveTool::read_entry(&ba2, "data/file-00255.txt").unwrap());
    });
    c.bench_function("extract_all_ba2_256x512", |b| {
        b.iter_batched(
            || unique_dir("extract-ba2-output"),
            |output| {
                let summary = ArchiveTool::extract_all(
                    &ba2,
                    &ExtractAllOptions {
                        output: Some(output.clone()),
                        overwrite: OverwriteMode::Fail,
                        fsync: false,
                    },
                )
                .unwrap();
                fs::remove_dir_all(output).unwrap();
                summary
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_create_and_add(c: &mut Criterion) {
    let input_root = unique_dir("create-input");
    write_input_tree(&input_root, 512, 1_024);

    c.bench_function("create_tes3_512x1k", |b| {
        b.iter_batched(
            || {
                let dir = unique_dir("create-output");
                fs::create_dir_all(&dir).unwrap();
                dir.join("out.bsa")
            },
            |output| {
                let count =
                    ArchiveTool::create(&output, &input_root, &CreateOptions::default()).unwrap();
                fs::remove_dir_all(output.parent().unwrap()).unwrap();
                count
            },
            BatchSize::SmallInput,
        );
    });

    let (_archive_dir, archive) = prepare_fixture("add-base", 512, 1_024);
    let replacement = unique_dir("add-replacement");
    fs::create_dir_all(replacement.join("textures")).unwrap();
    fs::write(
        replacement.join("textures/file-00511.dds"),
        make_payload(999, 1_024),
    )
    .unwrap();

    c.bench_function("add_replace_one_tes3_512x1k", |b| {
        b.iter_batched(
            || {
                let dir = unique_dir("add-output");
                fs::create_dir_all(&dir).unwrap();
                dir.join("out.bsa")
            },
            |output| {
                let count = ArchiveTool::add(
                    &archive,
                    &AddOptions {
                        inputs: vec![replacement.clone()],
                        output: Some(output.clone()),
                        fsync: false,
                        follow_symlinks: false,
                    },
                )
                .unwrap();
                fs::remove_dir_all(output.parent().unwrap()).unwrap();
                count
            },
            BatchSize::SmallInput,
        );
    });

    let (_large_dir, large_archive) = prepare_fixture("add-base-large", 10_000, 64);
    let large_replacement = unique_dir("add-large-replacement");
    fs::create_dir_all(large_replacement.join("textures")).unwrap();
    fs::write(
        large_replacement.join("textures/file-09999.dds"),
        make_payload(42, 64),
    )
    .unwrap();

    c.bench_function("add_replace_one_tes3_10000x64", |b| {
        b.iter_batched(
            || {
                let dir = unique_dir("add-large-output");
                fs::create_dir_all(&dir).unwrap();
                dir.join("out.bsa")
            },
            |output| {
                let count = ArchiveTool::add(
                    &large_archive,
                    &AddOptions {
                        inputs: vec![large_replacement.clone()],
                        output: Some(output.clone()),
                        fsync: false,
                        follow_symlinks: false,
                    },
                )
                .unwrap();
                fs::remove_dir_all(output.parent().unwrap()).unwrap();
                count
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_list_and_lookup,
    bench_extract_all,
    bench_tes4_and_ba2,
    bench_create_and_add
);
criterion_main!(benches);
