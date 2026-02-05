use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use crate::project_management::config::models::{Module, CodeFile, Project};

/// Go-specific package generator
pub struct GoPackageGenerator;

impl GoPackageGenerator {
    /// Check if a file is a Go code file (ends with .go)
    fn is_go_code_file(filename: &str) -> bool {
        filename.ends_with(".go")
    }

    /// Generate Go module structure recursively
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

        // Generate code files in this module
        for codefile in module.files() {
            let filename = codefile.filename_with_extension("go");
            let file_path = module_path.join(&filename);

            // Create file (only if it doesn't exist)
            if !file_path.exists() {
                // Only add package declaration for Go code files
                let content = if Self::is_go_code_file(&filename) {
                    let package_name = Self::get_package_name_for_module(&module_name, codefile.name());
                    Self::generate_go_file_content(&package_name)
                } else {
                    String::new() // Non-Go files get no content
                };

                fs::write(&file_path, content)
                    .with_context(|| format!("Failed to create file: {}", file_path.display()))?;
            }
        }

        // Process submodules recursively
        for submodule in module.subtree() {
            let mut new_parent_modules = parent_modules.to_vec();
            new_parent_modules.push(module.name().to_string());
            
            Self::generate_module(&module_path, submodule, &new_parent_modules)?;
        }

        Ok(())
    }

    /// Generate go.mod file for Go project
    pub fn generate_go_mod<P: AsRef<Path>>(
        project_path: P,
        project_name: &str,
    ) -> Result<()> {
        let go_mod_path = project_path.as_ref().join("go.mod");
        let go_mod_content = Self::generate_go_mod_content(project_name);
        
        // Only create go.mod if it doesn't already exist
        if !go_mod_path.exists() {
            fs::write(&go_mod_path, go_mod_content)
                .with_context(|| format!("Failed to create go.mod: {}", go_mod_path.display()))?;
        }

        Ok(())
    }

    /// Generate go.sum file for Go project
    pub fn generate_go_sum<P: AsRef<Path>>(
        project_path: P,
    ) -> Result<()> {
        let go_sum_path = project_path.as_ref().join("go.sum");
        
        // Create empty go.sum file (only if it doesn't exist)
        if !go_sum_path.exists() {
            fs::write(&go_sum_path, "")
                .with_context(|| format!("Failed to create go.sum: {}", go_sum_path.display()))?;
        }

        Ok(())
    }

    /// Generate main.go file for Go project
    pub fn generate_main_go<P: AsRef<Path>>(
        project_path: P,
    ) -> Result<()> {
        let main_go_path = project_path.as_ref().join("main.go");
        let main_content = Self::generate_main_go_content();
        
        // Only create main.go if it doesn't already exist
        if !main_go_path.exists() {
            fs::write(&main_go_path, main_content)
                .with_context(|| format!("Failed to create main.go: {}", main_go_path.display()))?;
        }

        Ok(())
    }

    /// Generate Go file content with package declaration
    fn generate_go_file_content(package_name: &str) -> String {
        format!("package {}\n\n", package_name)
    }

    /// Get appropriate package name for Go file based on module and file name
    fn get_package_name_for_module(module_name: &str, file_name: &str) -> String {
        // Use module name as package name (Go convention)
        Self::sanitize_package_name(module_name)
    }

    /// Sanitize package name for Go (convert hyphens to underscores, make lowercase)
    fn sanitize_package_name(name: &str) -> String {
        name.replace('-', "_").to_lowercase()
    }

    /// Generate go.mod file content
    fn generate_go_mod_content(project_name: &str) -> String {
        format!(
            "module {}\n\ngo 1.21\n",
            project_name
        )
    }

    /// Generate main.go file content
    fn generate_main_go_content() -> String {
        r#"package main

import "fmt"

func main() {
    fmt.Println("Hello, world!")
}
"#.to_string()
    }

    /// Check if project should have main.go
    pub fn should_generate_main_go(project: &Project) -> bool {
        // Generate main.go if there's no explicit main.go file defined in the project
        let has_main_in_project = project.files().iter()
            .any(|f| f.name() == "main" || f.filename_with_extension("go") == "main.go");
        
        let has_main_in_modules = project.tree().iter()
            .flat_map(|m| Self::find_main_in_module(m))
            .any(|has_main| has_main);
        
        !has_main_in_project && !has_main_in_modules
    }

    /// Recursively check if module contains main.go
    fn find_main_in_module(module: &Module) -> Vec<bool> {
        let mut results = vec![];
        
        // Check files in current module
        let has_main = module.files().iter()
            .any(|f| f.name() == "main" || f.filename_with_extension("go") == "main.go");
        results.push(has_main);
        
        // Check submodules recursively
        for submodule in module.subtree() {
            results.extend(Self::find_main_in_module(submodule));
        }
        
        results
    }
}