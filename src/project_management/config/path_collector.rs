use crate::project_management::config::models::{MoliConfig, Module};

/// Represents a file or directory managed by moli.yml
#[derive(Debug, Clone)]
pub struct ManagedFile {
    /// Display path (e.g., "src/domain/model.rs" or "src/domain/")
    pub display_path: String,
    /// Index of the project in the config
    pub project_index: usize,
    /// File or module name as written in moli.yml
    pub file_name: String,
    /// Module path leading to this entry (e.g., ["src", "domain"])
    pub module_path: Vec<String>,
    /// Whether this file is at the project level (not inside a tree)
    pub is_project_level: bool,
    /// Whether this entry is a directory (module) rather than a file
    pub is_directory: bool,
}

pub struct PathCollector;

impl PathCollector {
    /// Collect all managed file and directory paths from the config
    pub fn collect_all_entries(config: &MoliConfig) -> Vec<ManagedFile> {
        let mut entries = Vec::new();

        for (project_index, project) in config.projects().iter().enumerate() {
            let base_path = if project.is_root() {
                String::new()
            } else {
                format!("{}/", project.name())
            };

            // Project-level files
            for codefile in project.files() {
                let filename = codefile.filename_with_extension(project.language());
                let display_path = format!("{}{}", base_path, filename);
                entries.push(ManagedFile {
                    display_path,
                    project_index,
                    file_name: codefile.name().to_string(),
                    module_path: vec![],
                    is_project_level: true,
                    is_directory: false,
                });
            }

            // Tree entries (recursive)
            for module in project.tree() {
                Self::collect_module_entries(
                    &base_path,
                    module,
                    project.language(),
                    project_index,
                    &[],
                    &mut entries,
                );
            }
        }

        entries
    }

    /// Collect only files (backward compatible)
    pub fn collect_all_files(config: &MoliConfig) -> Vec<ManagedFile> {
        Self::collect_all_entries(config)
            .into_iter()
            .filter(|e| !e.is_directory)
            .collect()
    }

    fn collect_module_entries(
        base_path: &str,
        module: &Module,
        language: &str,
        project_index: usize,
        parent_modules: &[String],
        entries: &mut Vec<ManagedFile>,
    ) {
        let module_name = module.name();
        let mut current_module_path = parent_modules.to_vec();
        current_module_path.push(module_name.clone());

        let module_dir = format!("{}{}/", base_path, current_module_path.join("/"));

        // Add directory entry for this module
        entries.push(ManagedFile {
            display_path: module_dir.clone(),
            project_index,
            file_name: module_name,
            module_path: parent_modules.to_vec(),
            is_project_level: false,
            is_directory: true,
        });

        // Files in this module
        for codefile in module.files() {
            let filename = codefile.filename_with_extension(language);
            let display_path = format!("{}{}", module_dir, filename);
            entries.push(ManagedFile {
                display_path,
                project_index,
                file_name: codefile.name().to_string(),
                module_path: current_module_path.clone(),
                is_project_level: false,
                is_directory: false,
            });
        }

        // Recurse into subtree
        for submodule in module.subtree() {
            Self::collect_module_entries(
                base_path,
                submodule,
                language,
                project_index,
                &current_module_path,
                entries,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_management::config::models::*;

    fn make_config(projects: Vec<Project>) -> MoliConfig {
        MoliConfig { projects }
    }

    #[test]
    fn test_collect_root_project_files() {
        let config = make_config(vec![Project {
            name: "app".to_string(),
            root: true,
            lang: "rust".to_string(),
            file: vec![],
            tree: vec![Module {
                name: Some("src".to_string()),
                from: None,
                r#pub: None,
                tree: vec![Module {
                    name: Some("domain".to_string()),
                    from: None,
                    r#pub: None,
                    tree: vec![],
                    file: vec![
                        CodeFile { name: "model".to_string(), r#pub: None },
                        CodeFile { name: "repository".to_string(), r#pub: None },
                    ],
                }],
                file: vec![
                    CodeFile { name: "main".to_string(), r#pub: None },
                ],
            }],
        }]);

        let files = PathCollector::collect_all_files(&config);
        let paths: Vec<&str> = files.iter().map(|f| f.display_path.as_str()).collect();

        assert_eq!(files.len(), 3);
        assert!(paths.contains(&"src/main.rs"));
        assert!(paths.contains(&"src/domain/model.rs"));
        assert!(paths.contains(&"src/domain/repository.rs"));
    }

    #[test]
    fn test_collect_entries_includes_directories() {
        let config = make_config(vec![Project {
            name: "app".to_string(),
            root: true,
            lang: "rust".to_string(),
            file: vec![],
            tree: vec![Module {
                name: Some("src".to_string()),
                from: None,
                r#pub: None,
                tree: vec![Module {
                    name: Some("domain".to_string()),
                    from: None,
                    r#pub: None,
                    tree: vec![],
                    file: vec![
                        CodeFile { name: "model".to_string(), r#pub: None },
                    ],
                }],
                file: vec![],
            }],
        }]);

        let entries = PathCollector::collect_all_entries(&config);
        let dirs: Vec<&str> = entries.iter()
            .filter(|e| e.is_directory)
            .map(|e| e.display_path.as_str())
            .collect();
        let files: Vec<&str> = entries.iter()
            .filter(|e| !e.is_directory)
            .map(|e| e.display_path.as_str())
            .collect();

        assert!(dirs.contains(&"src/"));
        assert!(dirs.contains(&"src/domain/"));
        assert!(files.contains(&"src/domain/model.rs"));
    }

    #[test]
    fn test_directory_module_path() {
        let config = make_config(vec![Project {
            name: "app".to_string(),
            root: true,
            lang: "rust".to_string(),
            file: vec![],
            tree: vec![Module {
                name: Some("src".to_string()),
                from: None,
                r#pub: None,
                tree: vec![Module {
                    name: Some("domain".to_string()),
                    from: None,
                    r#pub: None,
                    tree: vec![],
                    file: vec![],
                }],
                file: vec![],
            }],
        }]);

        let entries = PathCollector::collect_all_entries(&config);
        let domain_dir = entries.iter()
            .find(|e| e.is_directory && e.file_name == "domain")
            .unwrap();

        // module_path for a directory is its parent modules
        assert_eq!(domain_dir.module_path, vec!["src"]);
        assert!(domain_dir.is_directory);
    }

    #[test]
    fn test_collect_non_root_project_files() {
        let config = make_config(vec![Project {
            name: "backend".to_string(),
            root: false,
            lang: "go".to_string(),
            file: vec![
                CodeFile { name: "main".to_string(), r#pub: None },
            ],
            tree: vec![Module {
                name: Some("pkg".to_string()),
                from: None,
                r#pub: None,
                tree: vec![],
                file: vec![
                    CodeFile { name: "handler".to_string(), r#pub: None },
                ],
            }],
        }]);

        let files = PathCollector::collect_all_files(&config);
        let paths: Vec<&str> = files.iter().map(|f| f.display_path.as_str()).collect();

        assert_eq!(files.len(), 2);
        assert!(paths.contains(&"backend/main.go"));
        assert!(paths.contains(&"backend/pkg/handler.go"));
    }

    #[test]
    fn test_collect_project_level_files() {
        let config = make_config(vec![Project {
            name: "docs".to_string(),
            root: true,
            lang: "any".to_string(),
            file: vec![
                CodeFile { name: "README.md".to_string(), r#pub: None },
            ],
            tree: vec![],
        }]);

        let files = PathCollector::collect_all_files(&config);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].display_path, "README.md");
        assert!(files[0].is_project_level);
    }

    #[test]
    fn test_module_path_tracking() {
        let config = make_config(vec![Project {
            name: "app".to_string(),
            root: true,
            lang: "rust".to_string(),
            file: vec![],
            tree: vec![Module {
                name: Some("src".to_string()),
                from: None,
                r#pub: None,
                tree: vec![Module {
                    name: Some("domain".to_string()),
                    from: None,
                    r#pub: None,
                    tree: vec![],
                    file: vec![
                        CodeFile { name: "model".to_string(), r#pub: None },
                    ],
                }],
                file: vec![],
            }],
        }]);

        let files = PathCollector::collect_all_files(&config);
        let model_file = files.iter().find(|f| f.file_name == "model").unwrap();

        assert_eq!(model_file.module_path, vec!["src", "domain"]);
        assert!(!model_file.is_project_level);
    }

    #[test]
    fn test_file_with_explicit_extension() {
        let config = make_config(vec![Project {
            name: "app".to_string(),
            root: true,
            lang: "typescript".to_string(),
            file: vec![],
            tree: vec![Module {
                name: Some("src".to_string()),
                from: None,
                r#pub: None,
                tree: vec![],
                file: vec![
                    CodeFile { name: "App.tsx".to_string(), r#pub: None },
                ],
            }],
        }]);

        let files = PathCollector::collect_all_files(&config);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].display_path, "src/App.tsx");
    }
}
