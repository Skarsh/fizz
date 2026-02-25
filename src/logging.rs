use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing_appender::non_blocking::{self, WorkerGuard};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::{BoxMakeWriter, MakeWriterExt};

const DEFAULT_LOG_FILTER: &str = "warn,fizz=info";
const DEFAULT_LOG_FORMAT: &str = "pretty";
const DEFAULT_LOG_OUTPUT: &str = "stderr";
const DEFAULT_LOG_FILE_PATH: &str = "logs/fizz.log";

static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

type InitResult = Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>;

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

fn env_filter_from_env() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER))
}

fn stderr_writer() -> BoxMakeWriter {
    BoxMakeWriter::new(std::io::stderr)
}

fn init_with_writer(format: LogFormat, env_filter: EnvFilter, writer: BoxMakeWriter) -> InitResult {
    match format {
        LogFormat::Pretty => tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_writer(writer)
            .try_init(),
        LogFormat::Json => tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .with_writer(writer)
            .try_init(),
    }
}

fn init_file_output(format: LogFormat, file_path: &Path, include_stderr: bool) -> InitResult {
    let fallback_message = if include_stderr {
        "using stderr only"
    } else {
        "using stderr instead"
    };

    match build_file_writer(file_path) {
        Ok((file_writer, guard)) => {
            let writer = if include_stderr {
                BoxMakeWriter::new(std::io::stderr.and(file_writer))
            } else {
                BoxMakeWriter::new(file_writer)
            };

            let init_result = init_with_writer(format, env_filter_from_env(), writer);
            if init_result.is_ok() {
                let _ = LOG_GUARD.set(guard);
            }
            init_result
        }
        Err(err) => {
            let mode = if include_stderr { "both" } else { "file" };
            eprintln!(
                "fizz: failed to initialize LOG_OUTPUT={} at '{}': {}; {}",
                mode,
                file_path.display(),
                err,
                fallback_message
            );
            init_with_writer(format, env_filter_from_env(), stderr_writer())
        }
    }
}

pub fn init() {
    let format = parse_log_format(env::var("LOG_FORMAT").ok().as_deref());
    let output = parse_log_output(env::var("LOG_OUTPUT").ok().as_deref());
    let file_path = parse_log_file_path(env::var("LOG_FILE_PATH").ok().as_deref());

    let init_result = match output {
        LogOutput::Stderr => init_with_writer(format, env_filter_from_env(), stderr_writer()),
        LogOutput::File => init_file_output(format, &file_path, false),
        LogOutput::Both => init_file_output(format, &file_path, true),
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
