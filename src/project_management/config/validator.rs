use anyhow::{bail, Result};
use crate::project_management::config::models::{MoliConfig, Project, Module};

/// Configuration validator for v2 moli.yml
pub struct ConfigValidator;

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub message: String,
    pub path: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Validation error at {}: {}", self.path, self.message)
    }
}

impl ConfigValidator {
    /// Validate entire moli configuration
    pub fn validate(config: &MoliConfig) -> Result<()> {
        let mut errors = Vec::new();

        // Check if at least one project exists
        if config.projects().is_empty() {
            bail!("Configuration must contain at least one project");
        }

        // Validate each project
        for (i, project) in config.projects().iter().enumerate() {
            if let Err(project_errors) = Self::validate_project(project, &format!("projects[{}]", i)) {
                errors.extend(project_errors);
            }
        }

        // Check root project constraints
        if let Err(root_error) = Self::validate_root_projects(config) {
            errors.push(root_error);
        }

        // Check project name uniqueness
        if let Err(name_errors) = Self::validate_project_names(config) {
            errors.extend(name_errors);
        }

        if !errors.is_empty() {
            let error_messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            bail!("Configuration validation failed:\n{}", error_messages.join("\n"));
        }

        Ok(())
    }

    /// Validate individual project
    fn validate_project(project: &Project, path: &str) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Check project name
        if project.name().is_empty() {
            errors.push(ValidationError {
                message: "Project name cannot be empty".to_string(),
                path: format!("{}.name", path),
            });
        }

        // Check language
        if project.language().is_empty() {
            errors.push(ValidationError {
                message: "Project language cannot be empty".to_string(),
                path: format!("{}.lang", path),
            });
        } else if !Self::is_supported_language(project.language()) {
            errors.push(ValidationError {
                message: format!("Unsupported language: {}", project.language()),
                path: format!("{}.lang", path),
            });
        }

        // Validate modules (tree)
        for (i, module) in project.tree().iter().enumerate() {
            if let Err(module_errors) = Self::validate_module(module, &format!("{}.tree[{}]", path, i), project.language()) {
                errors.extend(module_errors);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate module structure
    fn validate_module(module: &Module, path: &str, language: &str) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Check that either name or from is provided
        if module.name.is_none() && module.from.is_none() {
            errors.push(ValidationError {
                message: "Module must have either 'name' or 'from' field".to_string(),
                path: path.to_string(),
            });
        }

        // Check module name
        if module.name().is_empty() {
            errors.push(ValidationError {
                message: "Module name cannot be empty".to_string(),
                path: format!("{}.name", path),
            });
        }

        // Check that module name doesn't contain invalid characters
        if module.name().contains('/') || module.name().contains('\\') {
            errors.push(ValidationError {
                message: "Module name cannot contain path separators".to_string(),
                path: format!("{}.name", path),
            });
        }

        // If from is specified, language must be "any"
        if module.from.is_some() && language != "any" {
            errors.push(ValidationError {
                message: format!("Module with 'from' field can only be used with 'lang: any' (current: {})", language),
                path: format!("{}.from", path),
            });
        }

        // If from is specified, tree and file must be empty
        if module.from.is_some() {
            if !module.subtree().is_empty() {
                errors.push(ValidationError {
                    message: "Module with 'from' field cannot have 'tree' (git clone target cannot have subdirectories)".to_string(),
                    path: format!("{}.tree", path),
                });
            }
            if !module.files().is_empty() {
                errors.push(ValidationError {
                    message: "Module with 'from' field cannot have 'file' (git clone target cannot have files)".to_string(),
                    path: format!("{}.file", path),
                });
            }
        }

        // Validate sub-modules (subtree)
        for (i, submodule) in module.subtree().iter().enumerate() {
            if let Err(submodule_errors) = Self::validate_module(submodule, &format!("{}.tree[{}]", path, i), language) {
                errors.extend(submodule_errors);
            }
        }

        // Validate that module has either subtree or files (not both is allowed)
        // This is not an error, just a note for completeness

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate root project constraints
    fn validate_root_projects(config: &MoliConfig) -> Result<(), ValidationError> {
        let root_projects: Vec<_> = config.projects().iter().filter(|p| p.is_root()).collect();

        match root_projects.len() {
            0 => Ok(()), // Multi-project mode is valid
            1 => Ok(()), // Single project mode is valid
            _ => Err(ValidationError {
                message: "Only one project can be marked as root".to_string(),
                path: "projects".to_string(),
            }),
        }
    }

    /// Validate project name uniqueness
    fn validate_project_names(config: &MoliConfig) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        for (i, project) in config.projects().iter().enumerate() {
            if !seen_names.insert(project.name()) {
                errors.push(ValidationError {
                    message: format!("Duplicate project name: {}", project.name()),
                    path: format!("projects[{}].name", i),
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Check if language is supported
    fn is_supported_language(lang: &str) -> bool {
        matches!(lang, "rust" | "go" | "python" | "javascript" | "typescript" | "any" | "bash" | "lua")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_management::config::models::*;

    #[test]
    fn test_valid_single_project() {
        let config = MoliConfig {
            projects: vec![Project {
                name: "app".to_string(),
                root: true,
                lang: "rust".to_string(),
                tree: vec![],
                file: vec![],
            }],
        };

        assert!(ConfigValidator::validate(&config).is_ok());
    }

    #[test]
    fn test_valid_multi_project() {
        let config = MoliConfig {
            projects: vec![
                Project {
                    name: "backend".to_string(),
                    root: false,
                    lang: "rust".to_string(),
                    tree: vec![],
                    file: vec![],
                },
                Project {
                    name: "frontend".to_string(),
                    root: false,
                    lang: "javascript".to_string(),
                    tree: vec![],
                    file: vec![],
                },
            ],
        };

        assert!(ConfigValidator::validate(&config).is_ok());
    }

    #[test]
    fn test_multiple_root_projects_invalid() {
        let config = MoliConfig {
            projects: vec![
                Project {
                    name: "app1".to_string(),
                    root: true,
                    lang: "rust".to_string(),
                    tree: vec![],
                    file: vec![],
                },
                Project {
                    name: "app2".to_string(),
                    root: true,
                    lang: "go".to_string(),
                    tree: vec![],
                    file: vec![],
                },
            ],
        };

        assert!(ConfigValidator::validate(&config).is_err());
    }

    #[test]
    fn test_duplicate_project_names_invalid() {
        let config = MoliConfig {
            projects: vec![
                Project {
                    name: "app".to_string(),
                    root: false,
                    lang: "rust".to_string(),
                    tree: vec![],
                    file: vec![],
                },
                Project {
                    name: "app".to_string(),
                    root: false,
                    lang: "go".to_string(),
                    tree: vec![],
                    file: vec![],
                },
            ],
        };

        assert!(ConfigValidator::validate(&config).is_err());
    }

    #[test]
    fn test_unsupported_language_invalid() {
        let config = MoliConfig {
            projects: vec![Project {
                name: "app".to_string(),
                root: true,
                lang: "cobol".to_string(),
                tree: vec![],
                file: vec![],
            }],
        };

        assert!(ConfigValidator::validate(&config).is_err());
    }
}
