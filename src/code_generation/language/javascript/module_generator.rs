use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use crate::project_management::config::models::{Module, CodeFile, Project};
use crate::shared::utils::content_updater::ContentUpdater;

/// JavaScript-specific module generator
pub struct JavaScriptModuleGenerator;

impl JavaScriptModuleGenerator {
    /// Check if a file is a JavaScript code file (ends with .js, .jsx, or .mjs)
    fn is_javascript_code_file(filename: &str) -> bool {
        filename.ends_with(".js") || filename.ends_with(".jsx") || filename.ends_with(".mjs")
    }

    /// Generate JavaScript module structure recursively
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

        // Collect all submodule names and code file names for index.js
        let mut export_declarations = Vec::new();

        // Generate code files in this module
        for codefile in module.files() {
            let filename = codefile.filename_with_extension("javascript");
            let file_path = module_path.join(&filename);
            
            // Create empty JavaScript file (only if it doesn't exist)
            if !file_path.exists() {
                fs::write(&file_path, "")
                    .with_context(|| format!("Failed to create file: {}", file_path.display()))?;
            }

            // Add to export declarations if it's a JavaScript code file and not index.js
            if Self::is_javascript_code_file(&filename) && filename != "index.js" {
                // Use actual filename for ES module resolution (preserve extensions like .jsx)
                let module_name = if codefile.name().contains('.') {
                    // If the codefile name already has an extension, use the full filename
                    filename.strip_suffix(&format!(".{}", filename.split('.').last().unwrap_or("js")))
                        .unwrap_or(&filename).to_string()
                } else {
                    // If no extension in codefile name, remove the added .js extension for import
                    codefile.name().to_string()
                };

                // Use the actual generated filename for the import
                let import_path = if codefile.name().contains('.') {
                    format!("./{}", filename)
                } else {
                    format!("./{}.js", module_name)
                };

                export_declarations.push(format!("export * from '{}';", import_path));
            }
        }

        // Process submodules recursively
        for submodule in module.subtree() {
            let mut new_parent_modules = parent_modules.to_vec();
            new_parent_modules.push(module.name().to_string());
            
            Self::generate_module(&module_path, submodule, &new_parent_modules)?;
            
            // Add submodule export declaration
            export_declarations.push(format!("export * from './{}/index.js';", submodule.name()));
        }

        // Generate index.js only if explicitly defined in codefile
        let has_explicit_index = module.files().iter()
            .any(|f| f.name() == "index" || f.filename_with_extension("javascript") == "index.js");

        if has_explicit_index {
            let index_js_path = module_path.join("index.js");
            ContentUpdater::update_js_index_file(&index_js_path, &export_declarations)?;
        }

        Ok(())
    }

    /// Get module name for export statement (removes extension)
    fn get_module_name_for_export(codefile: &CodeFile) -> String {
        let name = codefile.name();
        // Remove common JavaScript extensions for proper module resolution
        if name.ends_with(".js") || name.ends_with(".mjs") || name.ends_with(".jsx") {
            let dot_pos = name.rfind('.').unwrap();
            name[..dot_pos].to_string()
        } else {
            name.to_string()
        }
    }

    /// Generate package.json for JavaScript project
    pub fn generate_package_json<P: AsRef<Path>>(
        project_path: P,
        project_name: &str,
    ) -> Result<()> {
        let package_json_path = project_path.as_ref().join("package.json");
        let package_content = Self::generate_package_json_content(project_name);
        
        // Only create package.json if it doesn't already exist
        if !package_json_path.exists() {
            fs::write(&package_json_path, package_content)
                .with_context(|| format!("Failed to create package.json: {}", package_json_path.display()))?;
        }

        Ok(())
    }

    /// Generate index.js file content
    fn generate_index_js_content(export_declarations: &[String]) -> String {
        format!(
            "// start auto exported by moli.\n{}\n// end auto exported by moli.\n\n",
            export_declarations.join("\n")
        )
    }

    /// Generate package.json content
    fn generate_package_json_content(project_name: &str) -> String {
        format!(
            r#"{{
  "name": "{}",
  "version": "1.0.0",
  "description": "",
  "main": "index.js",
  "type": "module",
  "scripts": {{
    "start": "node index.js",
    "dev": "node --watch index.js",
    "test": "echo \"Error: no test specified\" && exit 1"
  }},
  "keywords": [],
  "author": "",
  "license": "ISC"
}}
"#,
            project_name
        )
    }

    /// Generate main index.js file for JavaScript project
    pub fn generate_main_index_js<P: AsRef<Path>>(
        project_path: P,
    ) -> Result<()> {
        let index_js_path = project_path.as_ref().join("index.js");
        let index_content = Self::generate_main_index_js_content();
        
        // Create main index.js with simple content
        if !index_js_path.exists() {
            fs::write(&index_js_path, index_content)
                .with_context(|| format!("Failed to create index.js: {}", index_js_path.display()))?;
        }

        Ok(())
    }

    /// Generate main index.js content
    fn generate_main_index_js_content() -> String {
        r#"console.log("Hello, world!");
"#.to_string()
    }

    /// Check if project should have main index.js
    pub fn should_generate_main_index_js(project: &Project) -> bool {
        // Generate index.js if there's no explicit index.js file defined in the project
        let has_index_in_project = project.files().iter()
            .any(|f| f.name() == "index" || f.filename_with_extension("javascript") == "index.js");
        
        let has_index_in_modules = project.tree().iter()
            .flat_map(|m| Self::find_index_in_module(m))
            .any(|has_index| has_index);
        
        !has_index_in_project && !has_index_in_modules
    }

    /// Recursively check if module contains index.js
    fn find_index_in_module(module: &Module) -> Vec<bool> {
        let mut results = vec![];
        
        // Check files in current module
        let has_index = module.files().iter()
            .any(|f| f.name() == "index" || f.filename_with_extension("javascript") == "index.js");
        results.push(has_index);
        
        // Check submodules recursively
        for submodule in module.subtree() {
            results.extend(Self::find_index_in_module(submodule));
        }
        
        results
    }
}