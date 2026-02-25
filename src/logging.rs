use std::env;
use tracing_subscriber::EnvFilter;

const DEFAULT_LOG_FILTER: &str = "warn,fizz=info";
const DEFAULT_LOG_FORMAT: &str = "pretty";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LogFormat {
    Pretty,
    Json,
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

pub fn init() {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));
    let format = parse_log_format(env::var("LOG_FORMAT").ok().as_deref());

    match format {
        LogFormat::Pretty => {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(std::io::stderr)
                .try_init();
        }
        LogFormat::Json => {
            let _ = tracing_subscriber::fmt()
                .json()
                .with_env_filter(env_filter)
                .with_writer(std::io::stderr)
                .try_init();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LogFormat, parse_log_format};

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
}
