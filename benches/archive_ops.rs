use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use rome_archivetool::{AddOptions, ArchiveTool, CreateOptions, ExtractAllOptions, OverwriteMode};

fn unique_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "rome-archivetool-bench-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn make_payload(index: usize, size: usize) -> Vec<u8> {
    (0..size)
        .map(|offset| (index.wrapping_add(offset) % 251) as u8)
        .collect()
}

fn write_tes3_archive(path: &Path, entries: usize, payload_size: usize) {
    let archive: ba2::tes3::Archive = (0..entries)
        .map(|index| {
            let name = format!("textures/file-{index:05}.dds");
            let payload = make_payload(index, payload_size);
            (
                ba2::tes3::ArchiveKey::from(name.into_bytes()),
                ba2::tes3::File::from(payload.into_boxed_slice()),
            )
        })
        .collect();
    let mut output = fs::File::create(path).unwrap();
    archive.write(&mut output).unwrap();
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

fn bench_list_and_lookup(c: &mut Criterion) {
    let (_dir, archive) = prepare_fixture("list-lookup", 1_000, 256);

    c.bench_function("list_tes3_1000_entries", |b| {
        b.iter(|| ArchiveTool::list(&archive).unwrap())
    });

    c.bench_function("read_entry_tes3_last_of_1000", |b| {
        b.iter(|| ArchiveTool::read_entry(&archive, "textures/file-00999.dds").unwrap())
    });

    c.bench_function("read_entry_tes3_missing_1000", |b| {
        b.iter(|| ArchiveTool::read_entry(&archive, "textures/missing.dds").unwrap_err())
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
                    },
                )
                .unwrap();
                fs::remove_dir_all(output).unwrap();
                summary
            },
            BatchSize::SmallInput,
        )
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
                    },
                )
                .unwrap();
                fs::remove_dir_all(output).unwrap();
                summary
            },
            BatchSize::SmallInput,
        )
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
        )
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
                        output: output.clone(),
                    },
                )
                .unwrap();
                fs::remove_dir_all(output.parent().unwrap()).unwrap();
                count
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    bench_list_and_lookup,
    bench_extract_all,
    bench_create_and_add
);
criterion_main!(benches);
