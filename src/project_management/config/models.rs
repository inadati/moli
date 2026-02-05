use serde::{Deserialize, Serialize};

/// v2 moli.yml configuration root
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoliConfig {
    #[serde(rename = "$value")]
    pub projects: Vec<Project>,
}

/// Individual project configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    #[serde(default)]
    pub root: bool,
    pub lang: String,
    #[serde(default)]
    pub file: Vec<CodeFile>,
    #[serde(default)]
    pub tree: Vec<Module>,
}

/// Module or directory structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub r#pub: Option<String>,
    #[serde(default)]
    pub tree: Vec<Module>,
    #[serde(default)]
    pub file: Vec<CodeFile>,
}

/// Individual code file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeFile {
    pub name: String,
    #[serde(default)]
    pub r#pub: Option<String>,
}

impl MoliConfig {
    /// Get all projects
    pub fn projects(&self) -> &[Project] {
        &self.projects
    }

    /// Get root project (single project mode)
    pub fn root_project(&self) -> Option<&Project> {
        self.projects.iter().find(|p| p.root)
    }

    /// Get non-root projects (multi-project mode)
    pub fn sub_projects(&self) -> Vec<&Project> {
        self.projects.iter().filter(|p| !p.root).collect()
    }

    /// Check if this is a single project configuration
    pub fn is_single_project(&self) -> bool {
        self.root_project().is_some()
    }
}

impl Project {
    /// Get project name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Check if this is a root project
    pub fn is_root(&self) -> bool {
        self.root
    }

    /// Get project language
    pub fn language(&self) -> &str {
        &self.lang
    }

    /// Get top-level modules (tree)
    pub fn tree(&self) -> &[Module] {
        &self.tree
    }

    /// Get all code files at project level
    pub fn files(&self) -> &[CodeFile] {
        &self.file
    }
}

impl Module {
    /// Get module name (resolved from name or from)
    pub fn name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| {
                self.from
                    .as_ref()
                    .map(|url| Self::extract_repo_name(url))
                    .unwrap_or_else(|| String::from("unknown"))
            })
    }

    /// Check if this module is a git clone target
    pub fn is_git_clone(&self) -> bool {
        self.from.is_some()
    }

    /// Get git repository URL
    pub fn git_url(&self) -> Option<&str> {
        self.from.as_deref()
    }

    /// Extract repository name from git URL
    /// Supports both HTTPS and SSH formats
    fn extract_repo_name(url: &str) -> String {
        // Remove .git suffix if present
        let url = url.strip_suffix(".git").unwrap_or(url);

        // Extract last component from path
        url.split('/')
            .last()
            .map(|s| s.to_string())
            .unwrap_or_else(|| String::from("unknown"))
    }

    /// Get sub-modules (subtree)
    pub fn subtree(&self) -> &[Module] {
        &self.tree
    }

    /// Get code files in this module
    pub fn files(&self) -> &[CodeFile] {
        &self.file
    }

    /// Check if this module has sub-modules
    pub fn has_subtree(&self) -> bool {
        !self.tree.is_empty()
    }

    /// Check if this module has code files
    pub fn has_files(&self) -> bool {
        !self.file.is_empty()
    }

    /// Get pub visibility setting for the module
    pub fn pub_setting(&self) -> Option<&str> {
        self.r#pub.as_deref()
    }
}

impl CodeFile {
    /// Get file name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get file name with extension based on language
    pub fn filename_with_extension(&self, language: &str) -> String {
        if self.name.contains('.') {
            // Already has extension
            self.name.clone()
        } else {
            // Add language-specific extension
            let extension = match language {
                "rust" => "rs",
                "go" => "go",
                "python" => "py",
                "javascript" => "js",
                "typescript" => "ts",
                "markdown" => "md",
                _ => "txt", // fallback
            };
            format!("{}.{}", self.name, extension)
        }
    }

    /// Get pub visibility setting
    pub fn pub_setting(&self) -> Option<&str> {
        self.r#pub.as_deref()
    }
}
