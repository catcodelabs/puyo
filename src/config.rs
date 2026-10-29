use std::fs;
use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct PuyoConfig {
    pub post_upgrade_hooks: Vec<String>,
}

impl PuyoConfig {
    pub fn load() -> Self {
        let mut config = PuyoConfig::default();
        
        let config_path = match dirs_config_path() {
            Some(p) => p,
            None => return config,
        };

        if !config_path.exists() {
            return config;
        }

        if let Ok(content) = fs::read_to_string(config_path) {
            let mut in_hooks_section = false;
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with('[') && line.ends_with(']') {
                    in_hooks_section = line == "[hooks]";
                    continue;
                }

                if in_hooks_section {
                    if let Some(idx) = line.find('=') {
                        let key = line[..idx].trim();
                        let val = line[idx + 1..].trim();
                        if key == "post_upgrade" {
                            config.post_upgrade_hooks = parse_toml_string_array(val);
                        }
                    }
                }
            }
        }

        config
    }
}

fn dirs_config_path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|p| p.join("puyo").join("config.toml"))
}

fn parse_toml_string_array(val: &str) -> Vec<String> {
    let mut results = Vec::new();
    let val = val.trim();
    if val.starts_with('[') && val.ends_with(']') {
        let inner = &val[1..val.len() - 1];
        // Simple comma split handling double quotes
        for part in inner.split(',') {
            let item = part.trim().trim_matches('"').trim_matches('\'');
            if !item.is_empty() {
                results.push(item.to_string());
            }
        }
    }
    results
}