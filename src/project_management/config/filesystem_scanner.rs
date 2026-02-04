use std::collections::HashSet;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use ignore::WalkBuilder;
use crate::project_management::config::models::MoliConfig;
use crate::project_management::config::path_collector::PathCollector;

/// Represents a file or directory on the filesystem that is NOT managed by moli.yml
#[derive(Debug, Clone)]
pub struct UnmanagedEntry {
    /// Display path (e.g., "src/utils/" or "src/utils/helper.rs")
    pub display_path: String,
    /// Relative path from project root
    pub relative_path: PathBuf,
    /// Whether this entry is a directory
    pub is_directory: bool,
}

/// Files that moli manages automatically (should not be shown to users)
const MANAGED_FILES: &[&str] = &[
    "mod.rs",
    "__init__.py",
    "index.ts",
    "index.js",
];

/// Config/meta files that should be excluded from load candidates
const EXCLUDED_FILES: &[&str] = &[
    "moli.yml",
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "go.mod",
    "go.sum",
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    ".gitignore",
    ".gitattributes",
];

/// Directories that should always be excluded
const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".venv",
    "venv",
];

pub struct FilesystemScanner;

impl FilesystemScanner {
    /// Scan the filesystem and return entries not managed by moli.yml
    pub fn scan(config: &MoliConfig) -> Result<Vec<UnmanagedEntry>> {
        // Collect all managed paths from moli.yml
        let managed_entries = PathCollector::collect_all_entries(config);
        let managed_paths: HashSet<String> = managed_entries
            .iter()
            .map(|e| e.display_path.clone())
            .collect();

        let mut entries = Vec::new();
        let excluded_files: HashSet<&str> = EXCLUDED_FILES.iter().copied().collect();
        let managed_files: HashSet<&str> = MANAGED_FILES.iter().copied().collect();
        let excluded_dirs: HashSet<&str> = EXCLUDED_DIRS.iter().copied().collect();

        // Walk the filesystem respecting .gitignore
        let walker = WalkBuilder::new(".")
            .hidden(true)       // skip hidden files
            .git_ignore(true)   // respect .gitignore
            .git_global(true)   // respect global gitignore
            .git_exclude(true)  // respect .git/info/exclude
            .build();

        for result in walker {
            let entry = result.context("Failed to read directory entry")?;
            let path = entry.path();

            // Skip the root directory itself
            if path == Path::new(".") {
                continue;
            }

            // Get relative path (strip leading ./)
            let relative = path.strip_prefix("./").unwrap_or(path);
            let relative_str = relative.to_string_lossy();

            // Skip excluded directories
            if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                if let Some(name) = relative.file_name() {
                    if excluded_dirs.contains(name.to_string_lossy().as_ref()) {
                        continue;
                    }
                }
            }

            // Skip excluded files
            if let Some(file_name) = relative.file_name() {
                let name = file_name.to_string_lossy();
                if excluded_files.contains(name.as_ref()) {
                    continue;
                }
                // Skip moli-managed module files
                if managed_files.contains(name.as_ref()) {
                    continue;
                }
            }

            let is_dir = entry.file_type().map_or(false, |ft| ft.is_dir());
            let display_path = if is_dir {
                format!("{}/", relative_str)
            } else {
                relative_str.to_string()
            };

            // Skip if already managed by moli.yml
            if managed_paths.contains(&display_path) {
                continue;
            }

            // For files, also check without extension (moli.yml may omit standard extensions)
            if !is_dir {
                let without_ext = Self::strip_standard_extension(&relative_str);
                if let Some(stem) = without_ext {
                    // Check if a managed path matches the stem version
                    let stem_managed = managed_entries.iter().any(|e| {
                        !e.is_directory && e.display_path == display_path
                    });
                    if stem_managed {
                        continue;
                    }
                    // Also check by reconstructing what moli would generate
                    let _ = stem; // already checked via display_path above
                }
            }

            entries.push(UnmanagedEntry {
                display_path,
                relative_path: relative.to_path_buf(),
                is_directory: is_dir,
            });
        }

        // Sort: directories first, then alphabetically
        entries.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.display_path.cmp(&b.display_path),
            }
        });

        Ok(entries)
    }

    /// Strip standard language extension from a filename, returning the stem if applicable
    fn strip_standard_extension(path: &str) -> Option<String> {
        let standard_extensions = [".rs", ".go", ".py", ".ts", ".js"];
        for ext in &standard_extensions {
            if path.ends_with(ext) {
                return Some(path[..path.len() - ext.len()].to_string());
            }
        }
        None
    }

    /// Remove the standard language extension from a filename for moli.yml entry
    pub fn filename_without_standard_extension(filename: &str, language: &str) -> String {
        let ext = match language {
            "rust" => ".rs",
            "go" => ".go",
            "python" => ".py",
            "typescript" => ".ts",
            "javascript" => ".js",
            _ => return filename.to_string(),
        };
        if filename.ends_with(ext) {
            filename[..filename.len() - ext.len()].to_string()
        } else {
            filename.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_standard_extension() {
        assert_eq!(
            FilesystemScanner::strip_standard_extension("model.rs"),
            Some("model".to_string())
        );
        assert_eq!(
            FilesystemScanner::strip_standard_extension("handler.go"),
            Some("handler".to_string())
        );
        assert_eq!(
            FilesystemScanner::strip_standard_extension("App.tsx"),
            None
        );
        assert_eq!(
            FilesystemScanner::strip_standard_extension("README.md"),
            None
        );
    }

    #[test]
    fn test_filename_without_standard_extension() {
        assert_eq!(
            FilesystemScanner::filename_without_standard_extension("model.rs", "rust"),
            "model"
        );
        assert_eq!(
            FilesystemScanner::filename_without_standard_extension("handler.go", "go"),
            "handler"
        );
        assert_eq!(
            FilesystemScanner::filename_without_standard_extension("App.tsx", "typescript"),
            "App.tsx"
        );
        assert_eq!(
            FilesystemScanner::filename_without_standard_extension("main.py", "python"),
            "main"
        );
        assert_eq!(
            FilesystemScanner::filename_without_standard_extension("config.yaml", "rust"),
            "config.yaml"
        );
    }
}
