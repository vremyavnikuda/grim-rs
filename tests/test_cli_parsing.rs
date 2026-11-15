use std::process::Command;

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_grim-rs"))
        .args(args)
        .output()
        .expect("failed to run grim-rs binary")
}

fn assert_stderr_contains(output: &std::process::Output, needle: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(needle),
        "stderr did not contain {:?}\nactual stderr:\n{}",
        needle,
        stderr
    );
}

#[test]
fn cli_help_short_prints_usage_without_wayland() {
    let output = run_cli(&["-h"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: grim [options...] [output-file]"));
    assert!(stdout.contains("-s <factor>"));
}

#[test]
fn cli_help_long_prints_usage_without_wayland() {
    let output = run_cli(&["--help"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: grim [options...] [output-file]"));
    assert!(stdout.contains("-t png|jpeg"));
}

#[test]
fn cli_fails_when_scale_argument_is_missing() {
    let output = run_cli(&["-s"]);
    assert!(!output.status.success());
    assert_stderr_contains(&output, "Error: -s requires an argument");
}

#[test]
fn cli_fails_when_geometry_argument_is_missing() {
    let output = run_cli(&["-g"]);
    assert!(!output.status.success());
    assert_stderr_contains(&output, "Error: -g requires an argument");
}

#[test]
fn cli_fails_when_type_argument_is_missing() {
    let output = run_cli(&["-t"]);
    assert!(!output.status.success());
    assert_stderr_contains(&output, "Error: -t requires an argument");
}

#[test]
fn cli_fails_for_invalid_filetype() {
    let output = run_cli(&["-t", "gif"]);
    assert!(!output.status.success());
    assert_stderr_contains(&output, "Error: invalid filetype: gif");
}

#[test]
fn cli_fails_for_invalid_quality_range() {
    let output = run_cli(&["-q", "101"]);
    assert!(!output.status.success());
    assert_stderr_contains(&output, "Error: JPEG quality must be between 0 and 100");
}

#[test]
fn cli_fails_for_invalid_compression_level_range() {
    let output = run_cli(&["-l", "10"]);
    assert!(!output.status.success());
    assert_stderr_contains(
        &output,
        "Error: PNG compression level must be between 0 and 9",
    );
}

#[test]
fn cli_fails_for_negative_compression_level() {
    let output = run_cli(&["-l", "-1"]);
    assert!(!output.status.success());
    assert_stderr_contains(
        &output,
        "Error: PNG compression level must be between 0 and 9",
    );
}

#[test]
fn cli_fails_for_non_numeric_scale() {
    let output = run_cli(&["-s", "abc"]);
    assert!(!output.status.success());
    assert_stderr_contains(&output, "Invalid scale factor");
}

#[test]
fn cli_fails_for_non_numeric_quality() {
    let output = run_cli(&["-q", "bad"]);
    assert!(!output.status.success());
    assert_stderr_contains(&output, "Invalid quality value");
}

#[test]
fn cli_fails_for_too_many_positional_arguments() {
    let output = run_cli(&["out1.png", "out2.png"]);
    assert!(!output.status.success());
    assert_stderr_contains(&output, "Error: too many arguments");
}

#[test]
fn cli_warns_when_cursor_flag_without_output_name() {
    // The warning is emitted before any Wayland connection is made, so this
    // works headless too (the run then fails with a connection error). On a
    // live session the capture succeeds and would write out.png into the
    // current directory — run in a temp dir and clean up afterwards.
    let temp_dir = std::env::temp_dir();
    let output = Command::new(env!("CARGO_BIN_EXE_grim-rs"))
        .args(["-c", "out.png"])
        .current_dir(&temp_dir)
        .output()
        .expect("failed to run grim-rs binary");
    assert_stderr_contains(&output, "Warning: -c is only supported together with -o");
    let _ = std::fs::remove_file(temp_dir.join("out.png"));
}
