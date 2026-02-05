use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use crate::project_management::config::models::{Module, CodeFile, Project};
use crate::shared::utils::content_updater::ContentUpdater;

/// Rust-specific module generator
pub struct RustModuleGenerator;

impl RustModuleGenerator {
    /// Check if a file is a Rust code file (ends with .rs)
    fn is_rust_code_file(filename: &str) -> bool {
        filename.ends_with(".rs")
    }

    /// Convert pub setting to Rust visibility prefix
    /// target_type: "main", "lib", or "mod"
    fn get_visibility_prefix(pub_setting: Option<&str>, target_type: &str) -> &'static str {
        match pub_setting {
            Some("yes") => "pub ",
            Some("no") => "",
            Some("crate") => "pub(crate) ",
            Some("super") => "pub(super) ",
            None => {
                // Use defaults based on target type
                match target_type {
                    "main" => "", // main.rs defaults to private
                    "lib" | "mod" => "pub ", // lib.rs and mod.rs default to public
                    _ => "pub ",
                }
            }
            _ => "pub ", // fallback to public for unknown values
        }
    }
    /// Generate Rust module structure recursively
    pub fn generate_module<P: AsRef<Path>>(
        base_path: P,
        module: &Module,
        parent_modules: &[String],
    ) -> Result<()> {
        let module_name = module.name();
        let module_path = base_path.as_ref().join(&module_name);

        // Create directory
        fs::create_dir_all(&module_path)
            .with_context(|| format!("Failed to create directory: {}", module_path.display()))?;

        // Collect all submodule names and code file names for mod.rs
        let mut module_declarations = Vec::new();

        // Generate code files in this module
        for codefile in module.files() {
            let filename = codefile.filename_with_extension("rust");
            let file_path = module_path.join(&filename);

            // Create empty file (only if it doesn't exist)
            if !file_path.exists() {
                fs::write(&file_path, "")
                    .with_context(|| format!("Failed to create file: {}", file_path.display()))?;
            }

            // Add to module declarations only if it's a Rust code file and not mod.rs
            if Self::is_rust_code_file(&filename) && filename != "mod.rs" {
                let module_name = codefile.name();
                let visibility = Self::get_visibility_prefix(codefile.pub_setting(), "mod");
                module_declarations.push(format!("{}mod {};", visibility, module_name));
            }
        }

        // Process subtree recursively
        for submodule in module.subtree() {
            let mut new_parent_modules = parent_modules.to_vec();
            new_parent_modules.push(module_name.clone());

            Self::generate_module(&module_path, submodule, &new_parent_modules)?;

            // Add submodule declaration using the submodule's pub setting
            let visibility = Self::get_visibility_prefix(submodule.pub_setting(), "mod");
            module_declarations.push(format!("{}mod {};", visibility, submodule.name()));
        }

        // Generate mod.rs for all modules except src (src modules use main.rs or lib.rs instead)
        if module_name != "src" {
            let mod_rs_path = module_path.join("mod.rs");
            ContentUpdater::update_rust_module_file(&mod_rs_path, &module_declarations, None)?;
        }

        Ok(())
    }

    /// Generate main.rs content for root project
    pub fn generate_main_rs<P: AsRef<Path>>(
        project_path: P,
        src_modules: &[Module],
    ) -> Result<()> {
        let main_rs_path = project_path.as_ref().join("src").join("main.rs");

        // Collect module declarations from src module contents
        let mut module_declarations = Vec::new();

        for src_module in src_modules {
            if src_module.name() == "src" {
                // Add declarations for subtree and code files within src
                for codefile in src_module.files() {
                    let filename = codefile.filename_with_extension("rust");
                    if Self::is_rust_code_file(&filename)
                        && filename != "mod.rs"
                        && filename != "main.rs"
                        && filename != "lib.rs" {
                        let visibility = Self::get_visibility_prefix(codefile.pub_setting(), "main");
                        module_declarations.push(format!("{}mod {};", visibility, codefile.name()));
                    }
                }

                for submodule in src_module.subtree() {
                    let visibility = Self::get_visibility_prefix(submodule.pub_setting(), "main");
                    module_declarations.push(format!("{}mod {};", visibility, submodule.name()));
                }
            }
        }

        ContentUpdater::update_rust_module_file(&main_rs_path, &module_declarations, None)?;

        Ok(())
    }

    /// Generate lib.rs content for library project
    pub fn generate_lib_rs<P: AsRef<Path>>(
        project_path: P,
        src_modules: &[Module],
    ) -> Result<()> {
        let lib_rs_path = project_path.as_ref().join("src").join("lib.rs");

        // Collect module declarations from src module contents
        let mut module_declarations = Vec::new();

        for src_module in src_modules {
            if src_module.name() == "src" {
                // Add declarations for subtree and code files within src
                for codefile in src_module.files() {
                    let filename = codefile.filename_with_extension("rust");
                    if Self::is_rust_code_file(&filename)
                        && filename != "mod.rs"
                        && filename != "main.rs"
                        && filename != "lib.rs" {
                        let visibility = Self::get_visibility_prefix(codefile.pub_setting(), "lib");
                        module_declarations.push(format!("{}mod {};", visibility, codefile.name()));
                    }
                }

                for submodule in src_module.subtree() {
                    let visibility = Self::get_visibility_prefix(submodule.pub_setting(), "lib");
                    module_declarations.push(format!("{}mod {};", visibility, submodule.name()));
                }
            }
        }

        ContentUpdater::update_rust_module_file(&lib_rs_path, &module_declarations, None)?;

        Ok(())
    }

    /// Generate mod.rs file content
    fn generate_mod_rs_content(module_declarations: &[String]) -> String {
        format!(
            "// start auto exported by moli.\n{}\n// end auto exported by moli.\n\n",
            module_declarations.join("\n")
        )
    }

    /// Generate main.rs file content
    fn generate_main_rs_content(module_declarations: &[String]) -> String {
        let mod_section = if module_declarations.is_empty() {
            String::new()
        } else {
            format!(
                "// start auto exported by moli.\n{}\n// end auto exported by moli.\n\n",
                module_declarations.join("\n")
            )
        };

        format!(
            "{}fn main() {{\n    println!(\"Hello, world!\");\n}}\n",
            mod_section
        )
    }

    /// Generate lib.rs file content
    fn generate_lib_rs_content(module_declarations: &[String]) -> String {
        if module_declarations.is_empty() {
            "// Library root\n".to_string()
        } else {
            format!(
                "// start auto exported by moli.\n{}\n// end auto exported by moli.\n\n",
                module_declarations.join("\n")
            )
        }
    }

    /// Determine if project should generate main.rs (only if explicitly specified)
    pub fn should_generate_main_rs(project: &Project) -> bool {
        let has_main_in_project = project.files().iter()
            .any(|f| f.name() == "main" || f.filename_with_extension("rust") == "main.rs");

        let has_main_in_src = project.tree().iter()
            .filter(|m| m.name() == "src")
            .flat_map(|m| m.files())
            .any(|f| f.name() == "main" || f.filename_with_extension("rust") == "main.rs");

        has_main_in_project || has_main_in_src
    }

    /// Determine if project should generate lib.rs (only if explicitly specified)
    pub fn should_generate_lib_rs(project: &Project) -> bool {
        let has_lib_in_project = project.files().iter()
            .any(|f| f.name() == "lib" || f.filename_with_extension("rust") == "lib.rs");

        let has_lib_in_src = project.tree().iter()
            .filter(|m| m.name() == "src")
            .flat_map(|m| m.files())
            .any(|f| f.name() == "lib" || f.filename_with_extension("rust") == "lib.rs");

        has_lib_in_project || has_lib_in_src
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::project_management::config::models::*;

    #[test]
    fn test_generate_simple_module() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        let module = Module {
            name: Some("domain".to_string()),
            from: None,
            r#pub: None,
            tree: vec![],
            file: vec![
                CodeFile { name: "model".to_string(), r#pub: None },
                CodeFile { name: "repository".to_string(), r#pub: None },
            ],
        };

        RustModuleGenerator::generate_module(base_path, &module, &[]).unwrap();

        // Check directory exists
        assert!(base_path.join("domain").exists());

        // Check files exist
        assert!(base_path.join("domain/model.rs").exists());
        assert!(base_path.join("domain/repository.rs").exists());
        assert!(base_path.join("domain/mod.rs").exists());

        // Check mod.rs content
        let mod_content = fs::read_to_string(base_path.join("domain/mod.rs")).unwrap();
        assert!(mod_content.contains("pub mod model;"));
        assert!(mod_content.contains("pub mod repository;"));
    }

    #[test]
    fn test_generate_nested_modules() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        let module = Module {
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
        };

        RustModuleGenerator::generate_module(base_path, &module, &[]).unwrap();

        // Check nested structure
        assert!(base_path.join("src").exists());
        assert!(base_path.join("src/domain").exists());
        assert!(base_path.join("src/domain/model.rs").exists());
        assert!(base_path.join("src/domain/mod.rs").exists());
        assert!(!base_path.join("src/mod.rs").exists()); // src should not have mod.rs

        // Check domain mod.rs content
        let domain_mod_content = fs::read_to_string(base_path.join("src/domain/mod.rs")).unwrap();
        assert!(domain_mod_content.contains("pub mod model;"));
    }

    #[test]
    fn test_main_rs_generation() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // Create src directory
        fs::create_dir_all(project_path.join("src")).unwrap();

        let modules = vec![
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
                        file: vec![],
                    },
                ],
                file: vec![],
            },
        ];

        RustModuleGenerator::generate_main_rs(project_path, &modules).unwrap();

        let main_rs_path = project_path.join("src/main.rs");
        assert!(main_rs_path.exists());

        let main_content = fs::read_to_string(main_rs_path).unwrap();
        assert!(main_content.contains("mod domain;"));
    }
}
