use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn run_with_logging_env(
    log_output: &str,
    log_format: &str,
    log_file_path: Option<&Path>,
) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fizz"));
    cmd.arg("hi")
        .env("MODEL_PROVIDER", "invalid")
        .env("RUST_LOG", "fizz=info")
        .env("LOG_OUTPUT", log_output)
        .env("LOG_FORMAT", log_format);

    if let Some(path) = log_file_path {
        cmd.env("LOG_FILE_PATH", path);
    } else {
        cmd.env_remove("LOG_FILE_PATH");
    }

    cmd.output().expect("failed to run fizz binary")
}

fn unique_temp_dir(suffix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "fizz-logging-{suffix}-{stamp}-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("failed to create temp directory");
    dir
}

fn find_rotated_log_file(dir: &Path, base_file_name: &str) -> PathBuf {
    let expected_prefix = format!("{base_file_name}.");
    let mut matches: Vec<PathBuf> = fs::read_dir(dir)
        .expect("failed to read temp directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with(&expected_prefix))
                .unwrap_or(false)
        })
        .collect();

    matches.sort();
    matches
        .pop()
        .expect("expected a rotated log file to be created")
}

#[test]
fn json_format_emits_json_log_lines_on_stderr() {
    let output = run_with_logging_env("stderr", "json", None);
    assert!(
        !output.status.success(),
        "invalid provider should fail command"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let json_lines: Vec<&str> = stderr
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
        .collect();
    assert!(
        !json_lines.is_empty(),
        "expected at least one JSON log line, got stderr:\n{stderr}"
    );

    let parsed: Vec<Value> = json_lines
        .iter()
        .map(|line| serde_json::from_str::<Value>(line).expect("line should be valid JSON"))
        .collect();
    assert!(
        parsed.iter().any(|entry| {
            entry
                .get("fields")
                .and_then(|fields| fields.get("message"))
                .and_then(Value::as_str)
                == Some("loaded runtime configuration")
        }),
        "expected startup log message in JSON output, got stderr:\n{stderr}"
    );
}

#[test]
fn file_output_writes_logs_to_rotated_file() {
    let dir = unique_temp_dir("file");
    let log_path = dir.join("fizz.log");
    let output = run_with_logging_env("file", "pretty", Some(&log_path));
    assert!(
        !output.status.success(),
        "invalid provider should fail command"
    );

    let rotated = find_rotated_log_file(&dir, "fizz.log");
    let file_contents = fs::read_to_string(&rotated).expect("failed to read rotated log file");
    assert!(
        file_contents.contains("loaded runtime configuration"),
        "expected startup log message in file, got:\n{file_contents}"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("loaded runtime configuration"),
        "did not expect normal logs on stderr for file-only mode:\n{stderr}"
    );
    assert!(
        stderr.contains("Unsupported MODEL_PROVIDER"),
        "expected command error output on stderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn both_output_writes_logs_to_stderr_and_file() {
    let dir = unique_temp_dir("both");
    let log_path = dir.join("fizz.log");
    let output = run_with_logging_env("both", "pretty", Some(&log_path));
    assert!(
        !output.status.success(),
        "invalid provider should fail command"
    );

    let rotated = find_rotated_log_file(&dir, "fizz.log");
    let file_contents = fs::read_to_string(&rotated).expect("failed to read rotated log file");
    assert!(
        file_contents.contains("loaded runtime configuration"),
        "expected startup log message in file, got:\n{file_contents}"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("loaded runtime configuration"),
        "expected startup log message on stderr, got:\n{stderr}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn invalid_file_path_falls_back_to_stderr_logging() {
    let dir = unique_temp_dir("fallback");
    let blocking_file = dir.join("not-a-directory");
    fs::write(&blocking_file, "block").expect("failed to create blocking file");
    let log_path = blocking_file.join("fizz.log");

    let output = run_with_logging_env("file", "pretty", Some(&log_path));
    assert!(
        !output.status.success(),
        "invalid provider should fail command"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to initialize LOG_OUTPUT=file"),
        "expected fallback warning, got:\n{stderr}"
    );
    assert!(
        stderr.contains("using stderr instead"),
        "expected stderr fallback message, got:\n{stderr}"
    );
    assert!(
        stderr.contains("loaded runtime configuration"),
        "expected logs to continue on stderr after fallback, got:\n{stderr}"
    );

    let _ = fs::remove_dir_all(&dir);
}
