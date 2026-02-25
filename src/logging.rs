use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing_appender::non_blocking::{self, WorkerGuard};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt;

const DEFAULT_LOG_FILTER: &str = "warn,fizz=info";
const DEFAULT_LOG_FORMAT: &str = "pretty";
const DEFAULT_LOG_OUTPUT: &str = "stderr";
const DEFAULT_LOG_FILE_PATH: &str = "logs/fizz.log";

static LOG_GUARDS: OnceLock<Vec<WorkerGuard>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LogFormat {
    Pretty,
    Json,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LogOutput {
    Stderr,
    File,
    Both,
}

fn parse_log_format(raw: Option<&str>) -> LogFormat {
    match raw
        .unwrap_or(DEFAULT_LOG_FORMAT)
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "json" => LogFormat::Json,
        _ => LogFormat::Pretty,
    }
}

fn parse_log_output(raw: Option<&str>) -> LogOutput {
    match raw
        .unwrap_or(DEFAULT_LOG_OUTPUT)
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "file" => LogOutput::File,
        "both" => LogOutput::Both,
        _ => LogOutput::Stderr,
    }
}

fn parse_log_file_path(raw: Option<&str>) -> PathBuf {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_LOG_FILE_PATH))
}

fn build_file_writer(path: &Path) -> std::io::Result<(non_blocking::NonBlocking, WorkerGuard)> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| std::ffi::OsStr::new("fizz.log"));

    fs::create_dir_all(dir)?;
    let appender = tracing_appender::rolling::daily(dir, file_name);
    Ok(tracing_appender::non_blocking(appender))
}

pub fn init() {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));
    let format = parse_log_format(env::var("LOG_FORMAT").ok().as_deref());
    let output = parse_log_output(env::var("LOG_OUTPUT").ok().as_deref());
    let file_path = parse_log_file_path(env::var("LOG_FILE_PATH").ok().as_deref());

    let init_result = match output {
        LogOutput::Stderr => match format {
            LogFormat::Pretty => tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(std::io::stderr)
                .try_init(),
            LogFormat::Json => tracing_subscriber::fmt()
                .json()
                .with_env_filter(env_filter)
                .with_writer(std::io::stderr)
                .try_init(),
        },
        LogOutput::File => match build_file_writer(&file_path) {
            Ok((file_writer, guard)) => {
                let init_result = match format {
                    LogFormat::Pretty => tracing_subscriber::fmt()
                        .with_env_filter(env_filter)
                        .with_writer(file_writer)
                        .try_init(),
                    LogFormat::Json => tracing_subscriber::fmt()
                        .json()
                        .with_env_filter(env_filter)
                        .with_writer(file_writer)
                        .try_init(),
                };
                if init_result.is_ok() {
                    let _ = LOG_GUARDS.set(vec![guard]);
                }
                init_result
            }
            Err(err) => {
                eprintln!(
                    "fizz: failed to initialize LOG_OUTPUT=file at '{}': {}; using stderr instead",
                    file_path.display(),
                    err
                );
                match format {
                    LogFormat::Pretty => tracing_subscriber::fmt()
                        .with_env_filter(env_filter)
                        .with_writer(std::io::stderr)
                        .try_init(),
                    LogFormat::Json => tracing_subscriber::fmt()
                        .json()
                        .with_env_filter(env_filter)
                        .with_writer(std::io::stderr)
                        .try_init(),
                }
            }
        },
        LogOutput::Both => match build_file_writer(&file_path) {
            Ok((file_writer, guard)) => {
                let tee_writer = std::io::stderr.and(file_writer);
                let init_result = match format {
                    LogFormat::Pretty => tracing_subscriber::fmt()
                        .with_env_filter(env_filter)
                        .with_writer(tee_writer)
                        .try_init(),
                    LogFormat::Json => tracing_subscriber::fmt()
                        .json()
                        .with_env_filter(env_filter)
                        .with_writer(tee_writer)
                        .try_init(),
                };
                if init_result.is_ok() {
                    let _ = LOG_GUARDS.set(vec![guard]);
                }
                init_result
            }
            Err(err) => {
                eprintln!(
                    "fizz: failed to initialize LOG_OUTPUT=both file path '{}': {}; using stderr only",
                    file_path.display(),
                    err
                );
                match format {
                    LogFormat::Pretty => tracing_subscriber::fmt()
                        .with_env_filter(env_filter)
                        .with_writer(std::io::stderr)
                        .try_init(),
                    LogFormat::Json => tracing_subscriber::fmt()
                        .json()
                        .with_env_filter(env_filter)
                        .with_writer(std::io::stderr)
                        .try_init(),
                }
            }
        },
    };

    let _ = init_result;
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        DEFAULT_LOG_FILE_PATH, LogFormat, LogOutput, parse_log_file_path, parse_log_format,
        parse_log_output,
    };

    #[test]
    fn parse_log_format_defaults_to_pretty() {
        assert_eq!(parse_log_format(None), LogFormat::Pretty);
    }

    #[test]
    fn parse_log_format_accepts_json() {
        assert_eq!(parse_log_format(Some("json")), LogFormat::Json);
        assert_eq!(parse_log_format(Some(" JSON ")), LogFormat::Json);
    }

    #[test]
    fn parse_log_format_falls_back_for_unknown_values() {
        assert_eq!(parse_log_format(Some("unknown")), LogFormat::Pretty);
    }

    #[test]
    fn parse_log_output_defaults_to_stderr() {
        assert_eq!(parse_log_output(None), LogOutput::Stderr);
    }

    #[test]
    fn parse_log_output_accepts_file_and_both() {
        assert_eq!(parse_log_output(Some("file")), LogOutput::File);
        assert_eq!(parse_log_output(Some(" BOTH ")), LogOutput::Both);
    }

    #[test]
    fn parse_log_output_falls_back_for_unknown_values() {
        assert_eq!(parse_log_output(Some("unknown")), LogOutput::Stderr);
    }

    #[test]
    fn parse_log_file_path_uses_default_for_missing_or_empty_values() {
        assert_eq!(
            parse_log_file_path(None),
            PathBuf::from(DEFAULT_LOG_FILE_PATH)
        );
        assert_eq!(
            parse_log_file_path(Some("  ")),
            PathBuf::from(DEFAULT_LOG_FILE_PATH)
        );
    }

    #[test]
    fn parse_log_file_path_preserves_explicit_value() {
        assert_eq!(
            parse_log_file_path(Some("custom/fizz.log")),
            PathBuf::from("custom/fizz.log")
        );
    }
}
