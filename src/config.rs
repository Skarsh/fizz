use std::env;

const DEFAULT_MODEL_PROVIDER: &str = "ollama";
const DEFAULT_MODEL: &str = "qwen2.5:3b";
const DEFAULT_MODEL_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_SYSTEM_PROMPT: &str = "You are a helpful assistant.";
const DEFAULT_MODEL_TIMEOUT_SECS: u64 = 60;
const DEFAULT_TOOL_TIMEOUT_SECS: u64 = 30;
const DEFAULT_TOOL_MEMORY_MB: u64 = 256;
const DEFAULT_TOOL_ALLOW_DIRECT_NETWORK: bool = false;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRuntime {
    Builtin,
    Wasm,
}

impl ToolRuntime {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Wasm => "wasm",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceFsMode {
    Host,
    Overlay,
    Agentfs,
}

impl WorkspaceFsMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Overlay => "overlay",
            Self::Agentfs => "agentfs",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResourceLimits {
    pub timeout_secs: u64,
    pub memory_mb: u64,
}

impl Default for ToolResourceLimits {
    fn default() -> Self {
        Self {
            timeout_secs: DEFAULT_TOOL_TIMEOUT_SECS,
            memory_mb: DEFAULT_TOOL_MEMORY_MB,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPolicy {
    pub allow_direct_network: bool,
    pub resource_limits: ToolResourceLimits,
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self {
            allow_direct_network: DEFAULT_TOOL_ALLOW_DIRECT_NETWORK,
            resource_limits: ToolResourceLimits::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub model_provider: String,
    pub model: String,
    pub model_base_url: String,
    pub system_prompt: String,
    pub model_timeout_secs: u64,
    pub tool_runtime: ToolRuntime,
    pub tool_timeout_secs: u64,
    pub tool_memory_mb: u64,
    pub tool_allow_direct_network: bool,
    pub workspace_fs_mode: WorkspaceFsMode,
    pub tool_policy: ToolPolicy,
}

impl Config {
    pub fn from_env() -> Self {
        Self::from_env_with(|key| env::var(key).ok())
    }

    fn from_env_with(mut get_var: impl FnMut(&str) -> Option<String>) -> Self {
        let model_base_url =
            get_var("MODEL_BASE_URL").unwrap_or_else(|| DEFAULT_MODEL_BASE_URL.to_string());
        let model_timeout_secs = parse_model_timeout_secs(get_var("MODEL_TIMEOUT_SECS").as_deref());
        let tool_runtime = parse_tool_runtime(get_var("TOOL_RUNTIME").as_deref());
        let tool_timeout_secs = parse_tool_timeout_secs(get_var("TOOL_TIMEOUT_SECS").as_deref());
        let tool_memory_mb = parse_tool_memory_mb(get_var("TOOL_MEMORY_MB").as_deref());
        let tool_allow_direct_network = parse_bool(
            get_var("TOOL_ALLOW_DIRECT_NETWORK").as_deref(),
            DEFAULT_TOOL_ALLOW_DIRECT_NETWORK,
        );
        let workspace_fs_mode = parse_workspace_fs_mode(get_var("WORKSPACE_FS_MODE").as_deref());
        let tool_policy = ToolPolicy {
            allow_direct_network: tool_allow_direct_network,
            resource_limits: ToolResourceLimits {
                timeout_secs: tool_timeout_secs,
                memory_mb: tool_memory_mb,
            },
        };

        Self {
            model_provider: get_var("MODEL_PROVIDER")
                .unwrap_or_else(|| DEFAULT_MODEL_PROVIDER.to_string()),
            model: get_var("MODEL").unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            model_base_url,
            system_prompt: get_var("SYSTEM_PROMPT")
                .unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string()),
            model_timeout_secs,
            tool_runtime,
            tool_timeout_secs,
            tool_memory_mb,
            tool_allow_direct_network,
            workspace_fs_mode,
            tool_policy,
        }
    }
}

fn parse_positive_u64(raw: Option<&str>, default: u64) -> u64 {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn parse_model_timeout_secs(raw: Option<&str>) -> u64 {
    parse_positive_u64(raw, DEFAULT_MODEL_TIMEOUT_SECS)
}

fn parse_tool_timeout_secs(raw: Option<&str>) -> u64 {
    parse_positive_u64(raw, DEFAULT_TOOL_TIMEOUT_SECS)
}

fn parse_tool_memory_mb(raw: Option<&str>) -> u64 {
    parse_positive_u64(raw, DEFAULT_TOOL_MEMORY_MB)
}

fn parse_bool(raw: Option<&str>, default: bool) -> bool {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("1" | "true" | "yes" | "on") => true,
        Some("0" | "false" | "no" | "off") => false,
        _ => default,
    }
}

fn parse_tool_runtime(raw: Option<&str>) -> ToolRuntime {
    match raw
        .unwrap_or("builtin")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "wasm" => ToolRuntime::Wasm,
        _ => ToolRuntime::Builtin,
    }
}

fn parse_workspace_fs_mode(raw: Option<&str>) -> WorkspaceFsMode {
    match raw.unwrap_or("host").trim().to_ascii_lowercase().as_str() {
        "overlay" => WorkspaceFsMode::Overlay,
        "agentfs" => WorkspaceFsMode::Agentfs,
        _ => WorkspaceFsMode::Host,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{
        Config, DEFAULT_MODEL, DEFAULT_MODEL_BASE_URL, DEFAULT_MODEL_PROVIDER,
        DEFAULT_MODEL_TIMEOUT_SECS, DEFAULT_SYSTEM_PROMPT, DEFAULT_TOOL_ALLOW_DIRECT_NETWORK,
        DEFAULT_TOOL_MEMORY_MB, DEFAULT_TOOL_TIMEOUT_SECS, ToolPolicy, ToolResourceLimits,
        ToolRuntime, WorkspaceFsMode, parse_bool, parse_model_timeout_secs, parse_tool_memory_mb,
        parse_tool_runtime, parse_tool_timeout_secs, parse_workspace_fs_mode,
    };

    fn config_from_pairs(pairs: &[(&str, &str)]) -> Config {
        let vars: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        Config::from_env_with(|key| vars.get(key).cloned())
    }

    #[test]
    fn from_env_uses_defaults_when_vars_are_missing() {
        let cfg = config_from_pairs(&[]);
        assert_eq!(cfg.model_provider, DEFAULT_MODEL_PROVIDER);
        assert_eq!(cfg.model, DEFAULT_MODEL);
        assert_eq!(cfg.model_base_url, DEFAULT_MODEL_BASE_URL);
        assert_eq!(cfg.system_prompt, DEFAULT_SYSTEM_PROMPT);
        assert_eq!(cfg.model_timeout_secs, DEFAULT_MODEL_TIMEOUT_SECS);
        assert_eq!(cfg.tool_runtime, ToolRuntime::Builtin);
        assert_eq!(cfg.tool_timeout_secs, DEFAULT_TOOL_TIMEOUT_SECS);
        assert_eq!(cfg.tool_memory_mb, DEFAULT_TOOL_MEMORY_MB);
        assert_eq!(
            cfg.tool_allow_direct_network,
            DEFAULT_TOOL_ALLOW_DIRECT_NETWORK
        );
        assert_eq!(cfg.workspace_fs_mode, WorkspaceFsMode::Host);
        assert_eq!(cfg.tool_policy, ToolPolicy::default());
    }

    #[test]
    fn from_env_reads_configured_values() {
        let cfg = config_from_pairs(&[
            ("MODEL_PROVIDER", "custom"),
            ("MODEL", "some-model:1"),
            ("MODEL_BASE_URL", "http://localhost:9999"),
            ("SYSTEM_PROMPT", "Be concise."),
            ("MODEL_TIMEOUT_SECS", "15"),
            ("TOOL_RUNTIME", "wasm"),
            ("TOOL_TIMEOUT_SECS", "9"),
            ("TOOL_MEMORY_MB", "512"),
            ("TOOL_ALLOW_DIRECT_NETWORK", "true"),
            ("WORKSPACE_FS_MODE", "overlay"),
        ]);

        assert_eq!(cfg.model_provider, "custom");
        assert_eq!(cfg.model, "some-model:1");
        assert_eq!(cfg.model_base_url, "http://localhost:9999");
        assert_eq!(cfg.system_prompt, "Be concise.");
        assert_eq!(cfg.model_timeout_secs, 15);
        assert_eq!(cfg.tool_runtime, ToolRuntime::Wasm);
        assert_eq!(cfg.tool_timeout_secs, 9);
        assert_eq!(cfg.tool_memory_mb, 512);
        assert!(cfg.tool_allow_direct_network);
        assert_eq!(cfg.workspace_fs_mode, WorkspaceFsMode::Overlay);
        assert_eq!(
            cfg.tool_policy,
            ToolPolicy {
                allow_direct_network: true,
                resource_limits: ToolResourceLimits {
                    timeout_secs: 9,
                    memory_mb: 512,
                },
            }
        );
    }

    #[test]
    fn from_env_uses_default_timeout_when_timeout_is_invalid() {
        let cfg = config_from_pairs(&[("MODEL_TIMEOUT_SECS", "0")]);
        assert_eq!(cfg.model_timeout_secs, DEFAULT_MODEL_TIMEOUT_SECS);
    }

    #[test]
    fn parse_model_timeout_secs_uses_default_for_missing_or_invalid_values() {
        assert_eq!(parse_model_timeout_secs(None), DEFAULT_MODEL_TIMEOUT_SECS);
        assert_eq!(
            parse_model_timeout_secs(Some("")),
            DEFAULT_MODEL_TIMEOUT_SECS
        );
        assert_eq!(
            parse_model_timeout_secs(Some("not-a-number")),
            DEFAULT_MODEL_TIMEOUT_SECS
        );
        assert_eq!(
            parse_model_timeout_secs(Some("0")),
            DEFAULT_MODEL_TIMEOUT_SECS
        );
    }

    #[test]
    fn parse_model_timeout_secs_accepts_positive_integer() {
        assert_eq!(parse_model_timeout_secs(Some("45")), 45);
        assert_eq!(parse_model_timeout_secs(Some("  90  ")), 90);
    }

    #[test]
    fn parse_tool_timeout_secs_uses_default_for_missing_or_invalid_values() {
        assert_eq!(parse_tool_timeout_secs(None), DEFAULT_TOOL_TIMEOUT_SECS);
        assert_eq!(
            parse_tool_timeout_secs(Some("not-a-number")),
            DEFAULT_TOOL_TIMEOUT_SECS
        );
        assert_eq!(
            parse_tool_timeout_secs(Some("0")),
            DEFAULT_TOOL_TIMEOUT_SECS
        );
    }

    #[test]
    fn parse_tool_timeout_secs_accepts_positive_integer() {
        assert_eq!(parse_tool_timeout_secs(Some("11")), 11);
    }

    #[test]
    fn parse_tool_memory_mb_uses_default_for_missing_or_invalid_values() {
        assert_eq!(parse_tool_memory_mb(None), DEFAULT_TOOL_MEMORY_MB);
        assert_eq!(
            parse_tool_memory_mb(Some("not-a-number")),
            DEFAULT_TOOL_MEMORY_MB
        );
        assert_eq!(parse_tool_memory_mb(Some("0")), DEFAULT_TOOL_MEMORY_MB);
    }

    #[test]
    fn parse_tool_memory_mb_accepts_positive_integer() {
        assert_eq!(parse_tool_memory_mb(Some("1024")), 1024);
    }

    #[test]
    fn parse_bool_respects_truthy_and_falsy_values() {
        assert!(parse_bool(Some("true"), false));
        assert!(parse_bool(Some(" YES "), false));
        assert!(!parse_bool(Some("off"), true));
        assert!(!parse_bool(Some(" 0 "), true));
    }

    #[test]
    fn parse_bool_returns_default_for_unknown_values() {
        assert!(parse_bool(Some("maybe"), true));
        assert!(!parse_bool(Some("maybe"), false));
        assert!(!parse_bool(None, false));
    }

    #[test]
    fn parse_tool_runtime_defaults_to_builtin_and_accepts_wasm() {
        assert_eq!(parse_tool_runtime(None), ToolRuntime::Builtin);
        assert_eq!(parse_tool_runtime(Some("unknown")), ToolRuntime::Builtin);
        assert_eq!(parse_tool_runtime(Some(" WASM ")), ToolRuntime::Wasm);
    }

    #[test]
    fn parse_workspace_fs_mode_defaults_to_host_and_accepts_known_values() {
        assert_eq!(parse_workspace_fs_mode(None), WorkspaceFsMode::Host);
        assert_eq!(
            parse_workspace_fs_mode(Some("unknown")),
            WorkspaceFsMode::Host
        );
        assert_eq!(
            parse_workspace_fs_mode(Some("overlay")),
            WorkspaceFsMode::Overlay
        );
        assert_eq!(
            parse_workspace_fs_mode(Some(" AGENTFS ")),
            WorkspaceFsMode::Agentfs
        );
    }

    #[test]
    fn from_env_uses_defaults_for_invalid_tool_settings() {
        let cfg = config_from_pairs(&[
            ("TOOL_RUNTIME", "native"),
            ("TOOL_TIMEOUT_SECS", "0"),
            ("TOOL_MEMORY_MB", "-1"),
            ("TOOL_ALLOW_DIRECT_NETWORK", "perhaps"),
            ("WORKSPACE_FS_MODE", "anything"),
        ]);

        assert_eq!(cfg.tool_runtime, ToolRuntime::Builtin);
        assert_eq!(cfg.tool_timeout_secs, DEFAULT_TOOL_TIMEOUT_SECS);
        assert_eq!(cfg.tool_memory_mb, DEFAULT_TOOL_MEMORY_MB);
        assert_eq!(
            cfg.tool_allow_direct_network,
            DEFAULT_TOOL_ALLOW_DIRECT_NETWORK
        );
        assert_eq!(cfg.workspace_fs_mode, WorkspaceFsMode::Host);
        assert_eq!(cfg.tool_policy, ToolPolicy::default());
    }
}
