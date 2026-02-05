use std::fs;
use std::path::Path;
use anyhow::{Context, Result};
use crate::project_management::config::models::{Project, Module};

/// File handler for "any" language - generates files with specified extensions only
pub struct AnyFileHandler;

impl AnyFileHandler {
    /// Generate complete project structure for "any" language
    /// - No project configuration files (no package.json, Cargo.toml, etc.)
    /// - No module management files (no mod.rs, index.ts, __init__.py, etc.)
    /// - Only generates files specified in codefile with their exact extensions
    pub fn generate_project<P: AsRef<Path>>(
        project_path: P,
        project: &Project,
    ) -> Result<()> {
        let project_path = project_path.as_ref();

        // Generate project-level code files (only if they don't exist)
        for codefile in project.files() {
            // For "any" language, use the exact filename (with extension if provided)
            let filename = if codefile.name().contains('.') {
                // Already has extension, use as-is
                codefile.name().to_string()
            } else {
                // No extension - user should provide one, but we'll create as-is
                codefile.name().to_string()
            };

            let file_path = project_path.join(&filename);

            // Only create file if it doesn't already exist
            if !file_path.exists() {
                fs::write(&file_path, "")
                    .with_context(|| format!("Failed to create file: {}", file_path.display()))?;
            }
        }

        // Generate module structure (directories and files)
        for module in project.tree() {
            Self::generate_module(project_path, module)?;
        }

        // Create empty README.md if it doesn't exist
        let readme_path = project_path.join("README.md");
        if !readme_path.exists() {
            fs::write(&readme_path, "")
                .with_context(|| format!("Failed to create README.md: {}", readme_path.display()))?;
        }

        Ok(())
    }

    /// Generate module structure recursively
    fn generate_module<P: AsRef<Path>>(
        parent_path: P,
        module: &Module,
    ) -> Result<()> {
        let parent_path = parent_path.as_ref();
        let module_name = module.name();
        let module_path = parent_path.join(&module_name);

        // If this is a git clone target
        if let Some(git_url) = module.from.as_ref() {
            // Check if directory already exists
            if module_path.exists() {
                eprintln!("⚠️  Directory already exists, skipping clone: {}", module_path.display());
                return Ok(());
            }

            // Execute git clone
            eprintln!("🔄 Cloning repository: {} -> {}", git_url, module_name);
            let output = std::process::Command::new("git")
                .arg("clone")
                .arg(git_url)
                .arg(&module_path)
                .output();

            match output {
                Ok(result) if result.status.success() => {
                    eprintln!("✅ Successfully cloned: {}", module_name);
                }
                Ok(result) => {
                    eprintln!("❌ Failed to clone {}: {}",
                        module_name,
                        String::from_utf8_lossy(&result.stderr));
                    eprintln!("⚠️  Continuing with remaining operations...");
                }
                Err(e) => {
                    eprintln!("❌ Failed to execute git clone for {}: {}", module_name, e);
                    eprintln!("⚠️  Continuing with remaining operations...");
                }
            }

            // Don't process subtree/files for git clone targets
            return Ok(());
        }

        // Create module directory
        fs::create_dir_all(&module_path)
            .with_context(|| format!("Failed to create module directory: {}", module_path.display()))?;

        // Generate code files in this module (only if they don't exist)
        for codefile in module.files() {
            let filename = if codefile.name().contains('.') {
                codefile.name().to_string()
            } else {
                codefile.name().to_string()
            };

            let file_path = module_path.join(&filename);

            if !file_path.exists() {
                fs::write(&file_path, "")
                    .with_context(|| format!("Failed to create file: {}", file_path.display()))?;
            }
        }

        // Recursively generate submodules
        for submodule in module.subtree() {
            Self::generate_module(&module_path, submodule)?;
        }

        Ok(())
    }
}
