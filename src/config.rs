use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliEntry {
    pub name: String,
    pub prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_storage_path")]
    pub storage_path: String,
}

fn default_storage_path() -> String {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("snipctl");
    data_dir
        .join("snippets.json")
        .to_string_lossy()
        .to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_general")]
    pub general: GeneralConfig,
    #[serde(default = "default_clis")]
    pub cli: Vec<CliEntry>,
}

fn default_general() -> GeneralConfig {
    GeneralConfig {
        storage_path: default_storage_path(),
    }
}

fn default_clis() -> Vec<CliEntry> {
    vec![
        CliEntry {
            name: "az".into(),
            prefix: "az".into(),
        },
        CliEntry {
            name: "aws".into(),
            prefix: "aws".into(),
        },
        CliEntry {
            name: "gcloud".into(),
            prefix: "gcloud".into(),
        },
    ]
}

impl Default for Config {
    fn default() -> Self {
        Config {
            general: default_general(),
            cli: default_clis(),
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("snipctl")
            .join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => {
                        eprintln!("Warning: failed to parse config: {e}");
                    }
                },
                Err(e) => {
                    eprintln!("Warning: failed to read config: {e}");
                }
            }
        }
        Config::default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self).expect("Failed to serialize config");
        fs::write(&path, content)
    }

    pub fn storage_path(&self) -> PathBuf {
        let p = &self.general.storage_path;
        if p.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(&p[2..]);
            }
        }
        PathBuf::from(p)
    }

    pub fn add_cli(&mut self, prefix: &str) {
        if !self.cli.iter().any(|c| c.prefix == prefix) {
            self.cli.push(CliEntry {
                name: prefix.into(),
                prefix: prefix.into(),
            });
        }
    }

    pub fn remove_cli(&mut self, prefix: &str) -> bool {
        let before = self.cli.len();
        self.cli.retain(|c| c.prefix != prefix);
        self.cli.len() < before
    }

    pub fn has_cli(&self, prefix: &str) -> bool {
        self.cli.iter().any(|c| c.prefix == prefix)
    }

    /// Detect CLI prefix from a command string
    pub fn detect_cli(&self, command: &str) -> Option<String> {
        let first_word = command.split_whitespace().next()?;
        for cli in &self.cli {
            if cli.prefix == first_word {
                return Some(cli.prefix.clone());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.cli.len(), 3);
        assert!(config.has_cli("az"));
        assert!(config.has_cli("aws"));
        assert!(config.has_cli("gcloud"));
    }

    #[test]
    fn test_add_remove_cli() {
        let mut config = Config::default();
        config.add_cli("gh");
        assert!(config.has_cli("gh"));
        assert_eq!(config.cli.len(), 4);

        config.add_cli("gh"); // duplicate
        assert_eq!(config.cli.len(), 4);

        assert!(config.remove_cli("gh"));
        assert!(!config.has_cli("gh"));
        assert_eq!(config.cli.len(), 3);
    }

    #[test]
    fn test_detect_cli() {
        let config = Config::default();
        assert_eq!(
            config.detect_cli("az group create --name foo"),
            Some("az".into())
        );
        assert_eq!(
            config.detect_cli("aws ec2 describe-instances"),
            Some("aws".into())
        );
        assert_eq!(
            config.detect_cli("gcloud compute instances list"),
            Some("gcloud".into())
        );
        assert_eq!(config.detect_cli("kubectl get pods"), None);
    }
}
