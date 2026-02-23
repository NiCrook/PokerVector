use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SiteKind {
    Acr,
}

impl fmt::Display for SiteKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SiteKind::Acr => write!(f, "ACR"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub site: SiteKind,
    pub hero: String,
    pub path: PathBuf,
    #[serde(default)]
    pub manual: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantConfig {
    #[serde(default = "default_qdrant_url")]
    pub url: String,
    #[serde(default = "default_collection")]
    pub collection: String,
}

fn default_qdrant_url() -> String {
    "http://localhost:6334".to_string()
}

fn default_collection() -> String {
    "poker_hands".to_string()
}

impl Default for QdrantConfig {
    fn default() -> Self {
        Self {
            url: default_qdrant_url(),
            collection: default_collection(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub qdrant: QdrantConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            accounts: Vec::new(),
            qdrant: QdrantConfig::default(),
        }
    }
}

/// Returns `~/.pokervector/data/` (LanceDB storage directory).
pub fn data_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".pokervector").join("data")
}

/// Returns `~/.pokervector/config.toml`.
pub fn config_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".pokervector").join("config.toml")
}

/// Load config from disk. Returns `Config::default()` if file missing.
pub fn load_config() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config at {}", path.display()))?;
    let config: Config = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config at {}", path.display()))?;
    Ok(config)
}

/// Load config from a specific path (for testing or override).
pub fn load_config_from(path: &Path) -> Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config at {}", path.display()))?;
    let config: Config = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config at {}", path.display()))?;
    Ok(config)
}

/// Write config to disk, creating `~/.pokervector/` if needed.
pub fn save_config(config: &Config) -> Result<()> {
    save_config_to(config, &config_path())
}

/// Write config to a specific path.
pub fn save_config_to(config: &Config, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory {}", parent.display()))?;
    }
    let content = toml::to_string_pretty(config).context("Failed to serialize config")?;
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write config to {}", path.display()))?;
    Ok(())
}

/// Merge scanned accounts into existing config.
/// New accounts are those whose `(site, hero)` pair doesn't already exist.
/// Returns the merged config and the list of newly added accounts.
pub fn merge_scanned(mut config: Config, scanned: Vec<Account>) -> (Config, Vec<Account>) {
    let mut new_accounts = Vec::new();
    for account in scanned {
        let exists = config.accounts.iter().any(|a| {
            a.site == account.site && a.hero == account.hero
        });
        if !exists {
            new_accounts.push(account.clone());
            config.accounts.push(account);
        }
    }
    (config, new_accounts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_round_trip() {
        let config = Config {
            accounts: vec![Account {
                site: SiteKind::Acr,
                hero: "PolarFox".to_string(),
                path: PathBuf::from(r"C:\AmericasCardroom\handHistory\PolarFox"),
                manual: false,
            }],
            qdrant: QdrantConfig::default(),
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.accounts.len(), 1);
        assert_eq!(parsed.accounts[0].hero, "PolarFox");
        assert_eq!(parsed.accounts[0].site, SiteKind::Acr);
        assert!(!parsed.accounts[0].manual);
        assert_eq!(parsed.qdrant.url, "http://localhost:6334");
        assert_eq!(parsed.qdrant.collection, "poker_hands");
    }

    #[test]
    fn test_load_missing_file() {
        let path = PathBuf::from("/nonexistent/path/config.toml");
        let config = load_config_from(&path).unwrap();
        assert!(config.accounts.is_empty());
        assert_eq!(config.qdrant.url, "http://localhost:6334");
    }

    #[test]
    fn test_save_and_load() {
        let tmp = NamedTempFile::new().unwrap();
        let config = Config {
            accounts: vec![Account {
                site: SiteKind::Acr,
                hero: "TestHero".to_string(),
                path: PathBuf::from("/tmp/test"),
                manual: true,
            }],
            qdrant: QdrantConfig {
                url: "http://custom:6334".to_string(),
                collection: "custom_hands".to_string(),
            },
        };

        save_config_to(&config, tmp.path()).unwrap();
        let loaded = load_config_from(tmp.path()).unwrap();

        assert_eq!(loaded.accounts.len(), 1);
        assert_eq!(loaded.accounts[0].hero, "TestHero");
        assert!(loaded.accounts[0].manual);
        assert_eq!(loaded.qdrant.url, "http://custom:6334");
        assert_eq!(loaded.qdrant.collection, "custom_hands");
    }

    #[test]
    fn test_merge_adds_new() {
        let config = Config {
            accounts: vec![Account {
                site: SiteKind::Acr,
                hero: "Existing".to_string(),
                path: PathBuf::from("/a"),
                manual: true,
            }],
            qdrant: QdrantConfig::default(),
        };

        let scanned = vec![
            Account {
                site: SiteKind::Acr,
                hero: "Existing".to_string(),
                path: PathBuf::from("/b"),
                manual: false,
            },
            Account {
                site: SiteKind::Acr,
                hero: "NewPlayer".to_string(),
                path: PathBuf::from("/c"),
                manual: false,
            },
        ];

        let (merged, new) = merge_scanned(config, scanned);
        assert_eq!(merged.accounts.len(), 2);
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].hero, "NewPlayer");
        // Existing account preserved with original path
        assert_eq!(merged.accounts[0].path, PathBuf::from("/a"));
    }

    #[test]
    fn test_merge_no_duplicates() {
        let config = Config {
            accounts: vec![Account {
                site: SiteKind::Acr,
                hero: "PolarFox".to_string(),
                path: PathBuf::from("/a"),
                manual: false,
            }],
            qdrant: QdrantConfig::default(),
        };

        let scanned = vec![Account {
            site: SiteKind::Acr,
            hero: "PolarFox".to_string(),
            path: PathBuf::from("/b"),
            manual: false,
        }];

        let (merged, new) = merge_scanned(config, scanned);
        assert_eq!(merged.accounts.len(), 1);
        assert!(new.is_empty());
    }

    #[test]
    fn test_defaults_when_missing() {
        let toml_str = r#"
[[accounts]]
site = "acr"
hero = "Test"
path = "/test"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(!config.accounts[0].manual); // default false
        assert_eq!(config.qdrant.url, "http://localhost:6334");
        assert_eq!(config.qdrant.collection, "poker_hands");
    }
}
