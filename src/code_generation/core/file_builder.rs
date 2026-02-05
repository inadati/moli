use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use crate::project_management::config::models::{Project, Module, CodeFile};
use crate::code_generation::language::rust::module_generator::RustModuleGenerator;
use crate::code_generation::language::typescript::module_generator::TypeScriptModuleGenerator;

/// File builder that handles language-specific file generation
pub struct FileBuilder;

impl FileBuilder {
    /// Build all files for a project
    pub fn build_project_files<P: AsRef<Path>>(
        base_path: P,
        project: &Project,
    ) -> Result<()> {
        let project_path = if project.is_root() {
            base_path.as_ref().to_path_buf()
        } else {
            base_path.as_ref().join(project.name())
        };

        match project.language() {
            "rust" => Self::build_rust_project_files(&project_path, project),
            "typescript" => Self::build_typescript_project_files(&project_path, project),
            _ => Self::build_generic_project_files(&project_path, project),
        }
    }

    /// Build Rust-specific project files
    fn build_rust_project_files<P: AsRef<Path>>(
        project_path: P,
        project: &Project,
    ) -> Result<()> {
        // Generate files in all modules
        for module in project.tree() {
            Self::build_rust_module_files(&project_path, module, &[])?;
        }

        // Generate main.rs or lib.rs for src modules (only if explicitly specified)
        if let Some(src_module) = project.tree().iter().find(|m| m.name() == "src") {
            if RustModuleGenerator::should_generate_main_rs(project) {
                RustModuleGenerator::generate_main_rs(&project_path, &[src_module.clone()])?;
            } else if RustModuleGenerator::should_generate_lib_rs(project) {
                RustModuleGenerator::generate_lib_rs(&project_path, &[src_module.clone()])?;
            }
            // If neither main nor lib is specified, don't generate either (e.g., DDD with src/bin/ structure)
        }

        Ok(())
    }

    /// Build TypeScript-specific project files
    fn build_typescript_project_files<P: AsRef<Path>>(
        project_path: P,
        project: &Project,
    ) -> Result<()> {
        // Generate package.json and tsconfig.json
        TypeScriptModuleGenerator::generate_package_json(&project_path, project.name())?;
        TypeScriptModuleGenerator::generate_tsconfig_json(&project_path)?;

        // Generate files in all modules
        for module in project.tree() {
            Self::build_typescript_module_files(&project_path, module, &[])?;
        }

        Ok(())
    }

    /// Build TypeScript module files recursively
    fn build_typescript_module_files<P: AsRef<Path>>(
        base_path: P,
        module: &Module,
        parent_modules: &[String],
    ) -> Result<()> {
        TypeScriptModuleGenerator::generate_module(base_path, module, parent_modules)
    }

    /// Build Rust module files recursively
    fn build_rust_module_files<P: AsRef<Path>>(
        base_path: P,
        module: &Module,
        parent_modules: &[String],
    ) -> Result<()> {
        RustModuleGenerator::generate_module(base_path, module, parent_modules)
    }

    /// Build generic project files (non-Rust languages)
    fn build_generic_project_files<P: AsRef<Path>>(
        project_path: P,
        project: &Project,
    ) -> Result<()> {
        for module in project.tree() {
            Self::build_generic_module_files(&project_path, module, project.language())?;
        }
        Ok(())
    }

    /// Build generic module files recursively
    fn build_generic_module_files<P: AsRef<Path>>(
        base_path: P,
        module: &Module,
        language: &str,
    ) -> Result<()> {
        let module_path = base_path.as_ref().join(module.name());

        // Generate code files in this module
        for codefile in module.files() {
            let filename = codefile.filename_with_extension(language);
            let file_path = module_path.join(&filename);
            
            // Create empty file with language-specific content if needed
            let content = Self::generate_file_content(codefile, language);
            fs::write(&file_path, content)
                .with_context(|| format!("Failed to create file: {}", file_path.display()))?;
        }

        // Process submodules recursively
        for submodule in module.subtree() {
            Self::build_generic_module_files(&module_path, submodule, language)?;
        }

        Ok(())
    }

    /// Generate initial content for files based on language
    fn generate_file_content(codefile: &CodeFile, language: &str) -> String {
        match language {
            "go" => {
                if codefile.name().contains("main") {
                    "package main\n\nfunc main() {\n\t// TODO: implement\n}\n".to_string()
                } else {
                    "package main\n\n// TODO: implement\n".to_string()
                }
            }
            "python" => {
                if codefile.name() == "__init__" {
                    "# Package initialization\n".to_string()
                } else {
                    "# TODO: implement\n".to_string()
                }
            }
            "javascript" => {
                if codefile.name().contains("index") {
                    "// Main entry point\nconsole.log('Hello, world!');\n".to_string()
                } else {
                    "// TODO: implement\n".to_string()
                }
            }
            "typescript" => {
                if codefile.name().contains("index") {
                    "// Main entry point\nconsole.log('Hello, world!');\n".to_string()
                } else {
                    "// TODO: implement\n".to_string()
                }
            }
            _ => {
                // Default empty content for other languages
                "".to_string()
            }
        }
    }

    /// Get all file paths that will be created for a project
    pub fn get_project_files(project: &Project) -> Vec<PathBuf> {
        let mut files = Vec::new();
        
        let base_path = if project.is_root() {
            PathBuf::new()
        } else {
            PathBuf::from(project.name())
        };

        for module in project.tree() {
            Self::collect_module_files(&base_path, module, project.language(), &mut files);
        }

        // Add language-specific files
        match project.language() {
            "rust" => {
                if let Some(_) = project.tree().iter().find(|m| m.name() == "src") {
                    files.push(base_path.join("src/main.rs"));
                }
            }
            "typescript" => {
                files.push(base_path.join("package.json"));
                files.push(base_path.join("tsconfig.json"));
            }
            _ => {}
        }

        files
    }

    /// Recursively collect all file paths for a module
    fn collect_module_files(
        base_path: &Path,
        module: &Module,
        language: &str,
        files: &mut Vec<PathBuf>,
    ) {
        let module_path = base_path.join(module.name());

        // Add code files
        for codefile in module.files() {
            let filename = codefile.filename_with_extension(language);
            files.push(module_path.join(filename));
        }

        // Add special files for language-specific modules
        match language {
            "rust" => {
                // Add mod.rs for Rust modules that have submodules or files (except src module)
                if module.name() != "src" && (!module.subtree().is_empty() || !module.files().is_empty()) {
                    files.push(module_path.join("mod.rs"));
                }
            }
            "typescript" => {
                // Add index.ts for TypeScript modules that have submodules or files
                if !module.subtree().is_empty() || !module.files().is_empty() {
                    files.push(module_path.join("index.ts"));
                }
            }
            _ => {}
        }

        // Process submodules
        for submodule in module.subtree() {
            Self::collect_module_files(&module_path, submodule, language, files);
        }
    }

    /// Check if all project files exist
    pub fn files_exist<P: AsRef<Path>>(
        base_path: P,
        project: &Project,
    ) -> bool {
        let files = Self::get_project_files(project);
        
        files.iter().all(|file| {
            let full_path = base_path.as_ref().join(file);
            full_path.exists() && full_path.is_file()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::project_management::config::models::*;
    use crate::code_generation::core::directory_builder::DirectoryBuilder;

    #[test]
    fn test_build_rust_project_files() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        let project = Project {
            name: "app".to_string(),
            root: true,
            lang: "rust".to_string(),
            file: vec![],
            tree: vec![
                Module {
                    name: Some("src".to_string()),
                    from: None,
                    r#pub: None,
                    tree: vec![
                        Module {
                            name: Some("domain".to_string()),
                            from: None,
                            r#pub: None,
                            tree: vec![],
                            file: vec![
                                CodeFile { name: "model".to_string(), r#pub: None },
                                CodeFile { name: "repository".to_string(), r#pub: None },
                            ],
                        },
                    ],
                    file: vec![
                        CodeFile { name: "main".to_string(), r#pub: None },
                    ],
                },
            ],
        };

        // Build directory structure first
        DirectoryBuilder::build_project_structure(base_path, &project).unwrap();

        // Build files
        FileBuilder::build_project_files(base_path, &project).unwrap();

        // Check files exist
        assert!(base_path.join("src/main.rs").exists());
        assert!(!base_path.join("src/mod.rs").exists()); // src should not have mod.rs
        assert!(base_path.join("src/domain/mod.rs").exists());
        assert!(base_path.join("src/domain/model.rs").exists());
        assert!(base_path.join("src/domain/repository.rs").exists());

        // Check mod.rs content
        let domain_mod = fs::read_to_string(base_path.join("src/domain/mod.rs")).unwrap();
        assert!(domain_mod.contains("pub mod model;"));
        assert!(domain_mod.contains("pub mod repository;"));

        // Check main.rs content includes module declarations
        let main_rs = fs::read_to_string(base_path.join("src/main.rs")).unwrap();
        assert!(main_rs.contains("mod domain;"));
    }

    #[test]
    fn test_build_go_project_files() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        let project = Project {
            name: "app".to_string(),
            root: true,
            lang: "go".to_string(),
            file: vec![],
            tree: vec![
                Module {
                    name: Some("src".to_string()),
                    from: None,
                    r#pub: None,
                    tree: vec![],
                    file: vec![
                        CodeFile { name: "main".to_string(), r#pub: None },
                        CodeFile { name: "utils".to_string(), r#pub: None },
                    ],
                },
            ],
        };

        // Build directory structure first
        DirectoryBuilder::build_project_structure(base_path, &project).unwrap();
        
        // Build files
        FileBuilder::build_project_files(base_path, &project).unwrap();

        // Check files exist
        assert!(base_path.join("src/main.go").exists());
        assert!(base_path.join("src/utils.go").exists());

        // Check content
        let main_go = fs::read_to_string(base_path.join("src/main.go")).unwrap();
        assert!(main_go.contains("package main"));
        assert!(main_go.contains("func main()"));

        let utils_go = fs::read_to_string(base_path.join("src/utils.go")).unwrap();
        assert!(utils_go.contains("package main"));
    }

    #[test]
    fn test_get_project_files() {
        let project = Project {
            name: "app".to_string(),
            root: true,
            lang: "rust".to_string(),
            file: vec![],
            tree: vec![
                Module {
                    name: Some("src".to_string()),
                    from: None,
                    r#pub: None,
                    tree: vec![
                        Module {
                            name: Some("domain".to_string()),
                            from: None,
                            r#pub: None,
                            tree: vec![],
                            file: vec![CodeFile { name: "model".to_string(), r#pub: None }],
                        },
                    ],
                    file: vec![],
                },
            ],
        };

        let files = FileBuilder::get_project_files(&project);
        
        assert!(files.contains(&PathBuf::from("src/main.rs")));
        assert!(!files.contains(&PathBuf::from("src/mod.rs"))); // src should not have mod.rs
        assert!(files.contains(&PathBuf::from("src/domain/mod.rs")));
        assert!(files.contains(&PathBuf::from("src/domain/model.rs")));
    }
}