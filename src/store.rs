/// JSON-backed snippet storage with multi-CLI support.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub cli: String,
    pub template: String,
    #[serde(default)]
    pub original: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_count")]
    pub usage_count: u32,
    #[serde(default = "now_str")]
    pub created_at: String,
    #[serde(default = "now_str")]
    pub last_used: String,
}

fn default_count() -> u32 {
    1
}

fn now_str() -> String {
    Utc::now().to_rfc3339()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetFile {
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
}

fn default_version() -> String {
    "1.0".into()
}

impl Default for SnippetFile {
    fn default() -> Self {
        SnippetFile {
            version: default_version(),
            snippets: Vec::new(),
        }
    }
}

pub struct SnippetStore {
    pub path: PathBuf,
}

impl SnippetStore {
    pub fn new(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if !path.exists() {
            let _ = Self::write_file(&path, &SnippetFile::default());
        }
        SnippetStore { path }
    }

    fn read_file(path: &Path) -> SnippetFile {
        match fs::read_to_string(path) {
            Ok(content) => {
                // Try versioned format first
                if let Ok(file) = serde_json::from_str::<SnippetFile>(&content) {
                    return file;
                }
                // Fall back to plain array (azsnip compat)
                if let Ok(snippets) = serde_json::from_str::<Vec<Snippet>>(&content) {
                    return SnippetFile {
                        version: "1.0".into(),
                        snippets,
                    };
                }
                SnippetFile::default()
            }
            Err(_) => SnippetFile::default(),
        }
    }

    fn write_file(path: &Path, file: &SnippetFile) -> std::io::Result<()> {
        let tmp = path.with_extension("tmp");
        let content = serde_json::to_string_pretty(file)?;
        fs::write(&tmp, content)?;
        fs::rename(&tmp, path)?; // atomic on most OS
        Ok(())
    }

    fn read(&self) -> SnippetFile {
        Self::read_file(&self.path)
    }

    fn write(&self, file: &SnippetFile) {
        if let Err(e) = Self::write_file(&self.path, file) {
            eprintln!("Error writing snippets: {e}");
        }
    }

    pub fn add(
        &self,
        template: &str,
        original: &str,
        description: &str,
        tags: Option<Vec<String>>,
        cli: &str,
    ) -> Snippet {
        let mut file = self.read();

        // dedup: if same template exists, bump usage_count
        if let Some(existing) = file.snippets.iter_mut().find(|s| s.template == template) {
            existing.usage_count += 1;
            existing.last_used = now_str();
            let result = existing.clone();
            self.write(&file);
            return result;
        }

        let tags = tags.unwrap_or_else(|| extract_tags(template, cli));
        let snippet = Snippet {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            cli: cli.to_string(),
            template: template.to_string(),
            original: original.to_string(),
            description: description.to_string(),
            tags,
            usage_count: 1,
            created_at: now_str(),
            last_used: now_str(),
        };
        file.snippets.push(snippet.clone());
        self.write(&file);
        snippet
    }

    pub fn get(&self, snippet_id: &str) -> Option<Snippet> {
        self.read()
            .snippets
            .into_iter()
            .find(|s| s.id == snippet_id)
    }

    pub fn all(&self) -> Vec<Snippet> {
        self.read().snippets
    }

    pub fn all_by_cli(&self, cli: &str) -> Vec<Snippet> {
        self.read()
            .snippets
            .into_iter()
            .filter(|s| s.cli == cli)
            .collect()
    }

    pub fn delete(&self, snippet_id: &str) -> bool {
        let mut file = self.read();
        let before = file.snippets.len();
        file.snippets.retain(|s| s.id != snippet_id);
        if file.snippets.len() < before {
            self.write(&file);
            true
        } else {
            false
        }
    }

    pub fn update(&self, snippet_id: &str, template: Option<&str>, description: Option<&str>, tags: Option<Vec<String>>) -> Option<Snippet> {
        let mut file = self.read();
        for s in &mut file.snippets {
            if s.id == snippet_id {
                if let Some(t) = template {
                    s.template = t.to_string();
                }
                if let Some(d) = description {
                    s.description = d.to_string();
                }
                if let Some(t) = tags {
                    s.tags = t;
                }
                let updated = s.clone();
                self.write(&file);
                return Some(updated);
            }
        }
        None
    }

    pub fn touch(&self, snippet_id: &str) {
        let mut file = self.read();
        for s in &mut file.snippets {
            if s.id == snippet_id {
                s.usage_count += 1;
                s.last_used = now_str();
                self.write(&file);
                return;
            }
        }
    }

    pub fn export_all(&self, cli_filter: Option<&str>) -> String {
        let file = self.read();
        let snippets: Vec<&Snippet> = match cli_filter {
            Some(cli) => file.snippets.iter().filter(|s| s.cli == cli).collect(),
            None => file.snippets.iter().collect(),
        };
        serde_json::to_string_pretty(&snippets).unwrap_or_default()
    }

    pub fn import_from(&self, data: &str, default_cli: &str) -> Result<u32, String> {
        let incoming: Vec<serde_json::Value> =
            serde_json::from_str(data).map_err(|e| format!("Invalid JSON: {e}"))?;

        let mut file = self.read();
        let existing: HashSet<String> = file.snippets.iter().map(|s| s.template.clone()).collect();
        let mut added = 0u32;

        for item in incoming {
            let template = item
                .get("template")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if template.is_empty() || existing.contains(&template) {
                continue;
            }

            let cli = item
                .get("cli")
                .and_then(|v| v.as_str())
                .unwrap_or(default_cli)
                .to_string();

            let snippet = Snippet {
                id: item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Uuid::new_v4().to_string()[..8].to_string()),
                cli: cli.clone(),
                template: template.clone(),
                original: item
                    .get("original")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                description: item
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                tags: item
                    .get("tags")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_else(|| extract_tags(&template, &cli)),
                usage_count: item
                    .get("usage_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                created_at: item
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&now_str())
                    .to_string(),
                last_used: item
                    .get("last_used")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&now_str())
                    .to_string(),
            };
            file.snippets.push(snippet);
            added += 1;
        }

        self.write(&file);
        Ok(added)
    }
}

/// Extract subcommands as tags, accounting for the CLI prefix.
pub fn extract_tags(template: &str, cli_prefix: &str) -> Vec<String> {
    let parts: Vec<&str> = template.split_whitespace().collect();
    let cli_parts: Vec<&str> = cli_prefix.split_whitespace().collect();
    let start_idx = cli_parts.len();

    let mut tags = Vec::new();
    for p in parts.iter().skip(start_idx) {
        if p.starts_with('-') || p.starts_with("{{") {
            break;
        }
        tags.push(p.to_string());
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_extract_tags_az() {
        let tags = extract_tags("az group create --name {{name}}", "az");
        assert_eq!(tags, vec!["group", "create"]);
    }

    #[test]
    fn test_extract_tags_aws() {
        let tags = extract_tags("aws ec2 describe-instances --instance-id {{id}}", "aws");
        assert_eq!(tags, vec!["ec2", "describe-instances"]);
    }

    #[test]
    fn test_extract_tags_gcloud() {
        let tags = extract_tags(
            "gcloud compute instances list --zone {{zone}}",
            "gcloud",
        );
        assert_eq!(tags, vec!["compute", "instances", "list"]);
    }

    #[test]
    fn test_store_add_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_snippets.json");
        let store = SnippetStore::new(path);

        let s = store.add(
            "az group create --name {{name}}",
            "az group create --name myRG",
            "",
            None,
            "az",
        );
        assert_eq!(s.cli, "az");
        assert_eq!(s.usage_count, 1);

        let got = store.get(&s.id).unwrap();
        assert_eq!(got.template, "az group create --name {{name}}");
    }

    #[test]
    fn test_store_dedup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_snippets.json");
        let store = SnippetStore::new(path);

        store.add("az group list", "az group list", "", None, "az");
        let s2 = store.add("az group list", "az group list", "", None, "az");
        assert_eq!(s2.usage_count, 2);
        assert_eq!(store.all().len(), 1);
    }

    #[test]
    fn test_store_delete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_snippets.json");
        let store = SnippetStore::new(path);

        let s = store.add("aws s3 ls", "aws s3 ls", "", None, "aws");
        assert!(store.delete(&s.id));
        assert!(store.get(&s.id).is_none());
    }

    #[test]
    fn test_store_filter_by_cli() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_snippets.json");
        let store = SnippetStore::new(path);

        store.add("az group list", "", "", None, "az");
        store.add("aws s3 ls", "", "", None, "aws");
        store.add("gcloud compute instances list", "", "", None, "gcloud");

        assert_eq!(store.all_by_cli("az").len(), 1);
        assert_eq!(store.all_by_cli("aws").len(), 1);
        assert_eq!(store.all().len(), 3);
    }
}
