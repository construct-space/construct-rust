pub mod manifest;
pub mod native;
pub mod wasm;

use std::path::PathBuf;

use manifest::{RuntimeType, SpaceManifest};
use native::NativeSpace;
use wasm::WasmSpace;

use crate::spaces::LoadedSpace;

pub struct SpaceLoader;

impl SpaceLoader {
    /// Returns the base directory for user-installed spaces: ~/.construct/spaces/
    fn spaces_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".construct").join("spaces"))
    }

    /// Scan ~/.construct/spaces/ and load all plugin spaces.
    /// Returns only plugin spaces (native + wasm). Built-in spaces are added by the caller.
    pub fn scan_plugin_spaces() -> Vec<LoadedSpace> {
        let Some(dir) = Self::spaces_dir() else {
            return Vec::new();
        };
        Self::scan_dir(&dir)
    }

    /// Scan a specific directory for plugin spaces.
    pub fn scan_dir(dir: &std::path::Path) -> Vec<LoadedSpace> {
        let mut spaces = Vec::new();

        if !dir.exists() {
            return spaces;
        }

        let Ok(entries) = std::fs::read_dir(dir) else {
            return spaces;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("space.toml");
            if !manifest_path.exists() {
                continue;
            }

            match SpaceManifest::from_file(&manifest_path) {
                Ok(manifest) => {
                    let lib_path = path.join(&manifest.runtime.library);
                    match manifest.runtime.runtime_type {
                        RuntimeType::Native => match NativeSpace::load(&lib_path) {
                            Ok(native) => {
                                eprintln!(
                                    "[construct] Loaded native space: {} ({})",
                                    manifest.space.name,
                                    lib_path.display()
                                );
                                spaces.push(LoadedSpace::Native(native));
                            }
                            Err(e) => {
                                eprintln!(
                                    "[construct] Failed to load native space '{}': {e}",
                                    manifest.space.name
                                );
                            }
                        },
                        RuntimeType::Wasm => match WasmSpace::load(&lib_path) {
                            Ok(wasm) => {
                                eprintln!(
                                    "[construct] Loaded WASM space: {} ({})",
                                    manifest.space.name,
                                    lib_path.display()
                                );
                                spaces.push(LoadedSpace::Wasm(wasm));
                            }
                            Err(e) => {
                                eprintln!(
                                    "[construct] Failed to load WASM space '{}': {e}",
                                    manifest.space.name
                                );
                            }
                        },
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[construct] Failed to read manifest {}: {e}",
                        manifest_path.display()
                    );
                }
            }
        }

        spaces
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_nonexistent_dir_returns_empty() {
        let spaces = SpaceLoader::scan_dir(&PathBuf::from("/nonexistent/path/spaces"));
        assert!(spaces.is_empty());
    }

    #[test]
    fn scan_empty_dir_returns_empty() {
        let dir = std::env::temp_dir().join("construct_test_empty_scan");
        let _ = std::fs::create_dir_all(&dir);
        let spaces = SpaceLoader::scan_dir(&dir);
        assert!(spaces.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_dir_skips_files_not_dirs() {
        let dir = std::env::temp_dir().join("construct_test_skip_files");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("not_a_dir.txt"), "hello").unwrap();
        let spaces = SpaceLoader::scan_dir(&dir);
        assert!(spaces.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_dir_skips_dir_without_manifest() {
        let dir = std::env::temp_dir().join("construct_test_no_manifest");
        let _ = std::fs::create_dir_all(dir.join("some-space"));
        let spaces = SpaceLoader::scan_dir(&dir);
        assert!(spaces.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_dir_gracefully_fails_on_missing_library() {
        // A valid manifest pointing to a library that doesn't exist
        // should not crash — just log and skip
        let dir = std::env::temp_dir().join("construct_test_missing_lib");
        let space_dir = dir.join("test-space");
        let _ = std::fs::create_dir_all(&space_dir);
        std::fs::write(
            space_dir.join("space.toml"),
            r#"
[space]
name = "Test"
version = "0.1.0"

[runtime]
type = "native"
library = "libtest.dylib"
"#,
        )
        .unwrap();

        let spaces = SpaceLoader::scan_dir(&dir);
        // Library doesn't exist, so loading fails gracefully — returns empty
        assert!(spaces.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_dir_gracefully_fails_on_invalid_manifest() {
        let dir = std::env::temp_dir().join("construct_test_bad_manifest");
        let space_dir = dir.join("bad-space");
        let _ = std::fs::create_dir_all(&space_dir);
        std::fs::write(space_dir.join("space.toml"), "not valid toml {{{{").unwrap();

        let spaces = SpaceLoader::scan_dir(&dir);
        assert!(spaces.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
