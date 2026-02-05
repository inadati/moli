use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use crate::project_management::config::models::{Module, CodeFile, Project};
use crate::shared::utils::content_updater::ContentUpdater;

/// Python-specific package generator
pub struct PythonPackageGenerator;

impl PythonPackageGenerator {
    /// Check if a file is a Python code file (ends with .py)
    fn is_python_code_file(filename: &str) -> bool {
        filename.ends_with(".py")
    }

    /// Generate Python module structure recursively
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

        // Collect import statements for __init__.py
        let mut import_statements = Vec::new();

        // Generate code files in this module
        for codefile in module.files() {
            let filename = codefile.filename_with_extension("python");
            let file_path = module_path.join(&filename);
            
            // Create empty Python file (only if it doesn't exist)
            if !file_path.exists() {
                fs::write(&file_path, "")
                    .with_context(|| format!("Failed to create file: {}", file_path.display()))?;
            }

            // Add to import statements if it's a Python code file and not __init__.py
            if Self::is_python_code_file(&filename) && filename != "__init__.py" {
                let module_name = Self::get_module_name_for_import(codefile);
                import_statements.push(format!("from .{} import *", module_name));
            }
        }

        // Process submodules recursively
        for submodule in module.subtree() {
            let mut new_parent_modules = parent_modules.to_vec();
            new_parent_modules.push(module.name().to_string());
            
            Self::generate_module(&module_path, submodule, &new_parent_modules)?;
            
            // Add submodule import statement
            import_statements.push(format!("from .{} import *", submodule.name()));
        }

        // Generate __init__.py (always create to mark as Python package)
        let init_py_path = module_path.join("__init__.py");
        ContentUpdater::update_python_init_file(&init_py_path, &import_statements)?;

        Ok(())
    }

    /// Get module name for import statement (removes .py extension)
    fn get_module_name_for_import(codefile: &CodeFile) -> String {
        let name = codefile.name();
        // Remove .py extension for proper Python import
        if name.ends_with(".py") {
            name[..name.len() - 3].to_string()
        } else {
            name.to_string()
        }
    }

    /// Generate requirements.txt for Python project
    pub fn generate_requirements_txt<P: AsRef<Path>>(
        project_path: P,
    ) -> Result<()> {
        let requirements_path = project_path.as_ref().join("requirements.txt");
        let requirements_content = Self::generate_requirements_content();
        
        // Only create requirements.txt if it doesn't already exist
        if !requirements_path.exists() {
            fs::write(&requirements_path, requirements_content)
                .with_context(|| format!("Failed to create requirements.txt: {}", requirements_path.display()))?;
        }

        Ok(())
    }

    /// Generate setup.py for Python project
    pub fn generate_setup_py<P: AsRef<Path>>(
        project_path: P,
        project_name: &str,
    ) -> Result<()> {
        let setup_py_path = project_path.as_ref().join("setup.py");
        let setup_content = Self::generate_setup_py_content(project_name);
        
        // Only create setup.py if it doesn't already exist
        if !setup_py_path.exists() {
            fs::write(&setup_py_path, setup_content)
                .with_context(|| format!("Failed to create setup.py: {}", setup_py_path.display()))?;
        }

        Ok(())
    }

    /// Generate main.py file for Python project
    pub fn generate_main_py<P: AsRef<Path>>(
        project_path: P,
    ) -> Result<()> {
        let main_py_path = project_path.as_ref().join("main.py");
        let main_content = Self::generate_main_py_content();
        
        // Only create main.py if it doesn't already exist
        if !main_py_path.exists() {
            fs::write(&main_py_path, main_content)
                .with_context(|| format!("Failed to create main.py: {}", main_py_path.display()))?;
        }

        Ok(())
    }

    /// Generate __init__.py file content
    fn generate_init_py_content(import_statements: &[String]) -> String {
        format!(
            "# start auto exported by moli.\n{}\n# end auto exported by moli.\n\n",
            import_statements.join("\n")
        )
    }

    /// Generate requirements.txt content
    fn generate_requirements_content() -> String {
        "# Add your Python dependencies here\n# Example:\n# requests>=2.25.0\n# numpy>=1.21.0\n".to_string()
    }

    /// Generate setup.py content
    fn generate_setup_py_content(project_name: &str) -> String {
        format!(
            r#"from setuptools import setup, find_packages

setup(
    name="{}",
    version="1.0.0",
    description="",
    packages=find_packages(),
    install_requires=[
        # Add your dependencies here
    ],
    python_requires=">=3.8",
    author="",
    author_email="",
    url="",
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
    ],
)
"#,
            project_name
        )
    }

    /// Generate main.py content
    fn generate_main_py_content() -> String {
        r#"#!/usr/bin/env python3
"""Main entry point for the application."""

def main():
    """Main function."""
    print("Hello, world!")

if __name__ == "__main__":
    main()
"#.to_string()
    }

    /// Check if project should have main.py
    pub fn should_generate_main_py(project: &Project) -> bool {
        // Generate main.py if there's no explicit main.py file defined in the project
        let has_main_in_project = project.files().iter()
            .any(|f| f.name() == "main" || f.filename_with_extension("python") == "main.py");
        
        let has_main_in_modules = project.tree().iter()
            .flat_map(|m| Self::find_main_in_module(m))
            .any(|has_main| has_main);
        
        !has_main_in_project && !has_main_in_modules
    }

    /// Recursively check if module contains main.py
    fn find_main_in_module(module: &Module) -> Vec<bool> {
        let mut results = vec![];
        
        // Check files in current module
        let has_main = module.files().iter()
            .any(|f| f.name() == "main" || f.filename_with_extension("python") == "main.py");
        results.push(has_main);
        
        // Check submodules recursively
        for submodule in module.subtree() {
            results.extend(Self::find_main_in_module(submodule));
        }
        
        results
    }
}