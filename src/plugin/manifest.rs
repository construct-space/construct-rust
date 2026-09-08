use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SpaceManifest {
    pub space: SpaceInfo,
    pub runtime: RuntimeInfo,
}

#[derive(Debug, Deserialize)]
pub struct SpaceInfo {
    pub name: String,
    #[allow(dead_code)]
    pub version: String,
    #[allow(dead_code)]
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RuntimeInfo {
    #[serde(rename = "type")]
    pub runtime_type: RuntimeType,
    pub library: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeType {
    Native,
    Wasm,
}

impl SpaceManifest {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read manifest: {}", path.display()))?;
        Self::from_str(&content)
            .with_context(|| format!("Failed to parse manifest: {}", path.display()))
    }

    pub fn from_str(content: &str) -> Result<Self> {
        let manifest: SpaceManifest = toml::from_str(content)?;
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_native_manifest() {
        let toml = r#"
[space]
name = "Design"
version = "0.1.0"
icon = "diamond"

[runtime]
type = "native"
library = "libdesign.dylib"
"#;
        let manifest = SpaceManifest::from_str(toml).unwrap();
        assert_eq!(manifest.space.name, "Design");
        assert_eq!(manifest.space.version, "0.1.0");
        assert_eq!(manifest.space.icon.as_deref(), Some("diamond"));
        assert_eq!(manifest.runtime.runtime_type, RuntimeType::Native);
        assert_eq!(manifest.runtime.library, "libdesign.dylib");
    }

    #[test]
    fn parse_wasm_manifest() {
        let toml = r#"
[space]
name = "Markdown Preview"
version = "0.2.0"

[runtime]
type = "wasm"
library = "module.wasm"
"#;
        let manifest = SpaceManifest::from_str(toml).unwrap();
        assert_eq!(manifest.space.name, "Markdown Preview");
        assert_eq!(manifest.runtime.runtime_type, RuntimeType::Wasm);
        assert_eq!(manifest.runtime.library, "module.wasm");
        assert!(manifest.space.icon.is_none());
    }

    #[test]
    fn parse_manifest_missing_runtime_type_fails() {
        let toml = r#"
[space]
name = "Bad"
version = "0.1.0"

[runtime]
library = "lib.dylib"
"#;
        assert!(SpaceManifest::from_str(toml).is_err());
    }

    #[test]
    fn parse_manifest_invalid_runtime_type_fails() {
        let toml = r#"
[space]
name = "Bad"
version = "0.1.0"

[runtime]
type = "python"
library = "lib.py"
"#;
        assert!(SpaceManifest::from_str(toml).is_err());
    }

    #[test]
    fn parse_manifest_missing_name_fails() {
        let toml = r#"
[space]
version = "0.1.0"

[runtime]
type = "native"
library = "lib.dylib"
"#;
        assert!(SpaceManifest::from_str(toml).is_err());
    }

    #[test]
    fn from_file_nonexistent_fails() {
        let result = SpaceManifest::from_file(Path::new("/nonexistent/space.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn from_file_valid() {
        let dir = std::env::temp_dir().join("construct_test_manifest");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("space.toml");
        std::fs::write(
            &path,
            r#"
[space]
name = "Test"
version = "1.0.0"

[runtime]
type = "native"
library = "libtest.dylib"
"#,
        )
        .unwrap();

        let manifest = SpaceManifest::from_file(&path).unwrap();
        assert_eq!(manifest.space.name, "Test");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
