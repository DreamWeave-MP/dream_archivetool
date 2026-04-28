// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "cli")]

use std::process::{Command, Output};

use tempfile::TempDir;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_dream-archivetool")
}

fn run(args: &[&str]) -> Output {
    Command::new(bin()).args(args).output().unwrap()
}

fn write_test_archive(dir: &TempDir) -> std::path::PathBuf {
    let archive = dir.path().join("test.bsa");
    let mut builder = dream_archive::Tes3BsaBuilder::new();
    builder
        .add_bytes("textures/example.dds", b"payload")
        .unwrap();
    builder.write_path(&archive).unwrap();
    archive
}

fn write_input_file(dir: &TempDir, name: &str) -> std::path::PathBuf {
    let input = dir.path().join(name);
    std::fs::write(&input, b"payload").unwrap();
    input
}

#[test]
fn no_subcommand_prints_help_to_stdout_successfully() {
    let output = run(&[]);

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    assert!(output.stderr.is_empty());
}

#[test]
fn clap_misuse_exits_nonzero_and_reports_to_stderr() {
    let output = run(&["extract"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("error:"));
}

#[test]
fn runtime_error_reports_to_stderr_and_exits_one() {
    let output = run(&["info", "missing.bsa"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("ERROR:"));
}

#[test]
fn extract_stdout_writes_exact_payload_bytes() {
    let dir = TempDir::new().unwrap();
    let archive = write_test_archive(&dir);
    let archive = archive.to_str().unwrap();
    let output = run(&["extract", archive, "textures/example.dds", "--stdout"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"payload");
    assert!(output.stderr.is_empty());
}

#[test]
fn dry_run_without_json_prints_json_plan() {
    let dir = TempDir::new().unwrap();
    let archive = write_test_archive(&dir);
    let archive = archive.to_str().unwrap();
    let output = run(&["extract-all", archive, "--dry-run"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["operation"], "extract-all");
    assert!(json["entries"].as_array().is_some());
}

#[test]
fn create_dry_run_without_json_prints_json_plan() {
    let dir = TempDir::new().unwrap();
    let input = write_input_file(&dir, "input.txt");
    let output_archive = dir.path().join("created.bsa");
    let output = run(&[
        "create",
        output_archive.to_str().unwrap(),
        input.to_str().unwrap(),
        "--format",
        "tes3",
        "--dry-run",
    ]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["operation"], "create");
    assert!(json["entries"].as_array().is_some());
}

#[test]
fn add_dry_run_without_json_prints_json_plan() {
    let dir = TempDir::new().unwrap();
    let archive = write_test_archive(&dir);
    let input = write_input_file(&dir, "new_file.txt");
    let output = run(&[
        "add",
        archive.to_str().unwrap(),
        input.to_str().unwrap(),
        "--dry-run",
    ]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["operation"], "add");
    assert!(json["entries"].as_array().is_some());
}
