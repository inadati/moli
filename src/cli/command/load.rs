use clap::Command;
use anyhow::{bail, Context, Result};
use inquire::{Select, Confirm};
use std::fs;
use std::path::Path;

use crate::project_management::config::{ConfigParser, ConfigValidator};
use crate::project_management::config::filesystem_scanner::{FilesystemScanner, UnmanagedEntry};
use crate::project_management::config::yaml_modifier::{YamlModifier, AddChild};
use crate::shared::utils::diff::show_diff;

pub fn spec() -> Command {
    Command::new("load")
        .about("Load unmanaged files or directories into moli.yml")
        .long_about(
            "Interactively select and add a file or directory to moli.yml.\n\
            \n\
            This command will:\n\
            1. Scan the filesystem for files/directories not in .gitignore\n\
            2. Filter out entries already managed by moli.yml\n\
            3. Let you search and select an entry to add\n\
            4. Generate the moli.yml entry (with diff preview)\n\
            5. Update moli.yml with confirmation\n\
            \n\
            Directories are shown with a trailing '/' and selecting one\n\
            will also add all files and subdirectories within it."
        )
}

pub fn action() -> Result<()> {
    // Check if moli.yml exists
    if !ConfigParser::config_exists() {
        bail!("moli.yml not found. Run 'moli new' to create a new project configuration.");
    }

    // Parse configuration
    let config = ConfigParser::parse_default()
        .context("Failed to parse moli.yml")?;

    // Validate configuration
    ConfigValidator::validate(&config)
        .context("Configuration validation failed")?;

    // Scan filesystem for unmanaged entries
    let all_entries = FilesystemScanner::scan(&config)
        .context("Failed to scan filesystem")?;

    if all_entries.is_empty() {
        println!("No unmanaged files or directories found.");
        return Ok(());
    }

    // Pre-filter: exclude entries that would produce no changes in moli.yml
    let yaml_content = fs::read_to_string("moli.yml")
        .context("Failed to read moli.yml")?;

    let unmanaged_entries: Vec<UnmanagedEntry> = all_entries
        .into_iter()
        .filter(|entry| would_produce_changes(&config, &yaml_content, entry))
        .collect();

    if unmanaged_entries.is_empty() {
        println!("No unmanaged files or directories found.");
        return Ok(());
    }

    // Build display options
    let display_options: Vec<String> = unmanaged_entries
        .iter()
        .map(|e| e.display_path.clone())
        .collect();

    // Interactive selection with fuzzy search
    let selected = Select::new("Select a file or directory to add to moli.yml:", display_options)
        .prompt()
        .context("Selection cancelled")?;

    // Find the selected entry
    let target = unmanaged_entries
        .iter()
        .find(|e| e.display_path == selected)
        .unwrap();

    // Determine which project to add to
    let (project_index, path_segments) = resolve_project(&config, target)?;
    let language = config.projects()[project_index].language();

    // Collect children if directory
    let children = if target.is_directory {
        collect_directory_children(&target.relative_path)?
    } else {
        vec![]
    };

    // Generate new YAML
    let new_yaml = if path_segments.is_empty() && target.is_directory {
        // Selected directory is the project root itself.
        // Add each child directly to the project.
        let mut result = yaml_content.clone();
        for child in &children {
            let child_path = vec![child.name.clone()];
            result = YamlModifier::add_entry(
                &result,
                project_index,
                &child_path,
                child.is_directory,
                language,
                &child.children,
            ).context("Failed to modify moli.yml")?;
        }
        result
    } else {
        YamlModifier::add_entry(
            &yaml_content,
            project_index,
            &path_segments,
            target.is_directory,
            language,
            &children,
        ).context("Failed to modify moli.yml")?
    };

    if yaml_content == new_yaml {
        println!("No changes needed. Entry may already exist in moli.yml.");
        return Ok(());
    }

    // Show diff
    println!();
    println!("Changes to moli.yml:");
    println!("---");
    show_diff(&yaml_content, &new_yaml);
    println!("---");

    let confirm = Confirm::new("Apply changes to moli.yml?")
        .with_default(true)
        .prompt()
        .context("Confirmation cancelled")?;

    if !confirm {
        println!("moli.yml was not modified.");
        return Ok(());
    }

    fs::write("moli.yml", &new_yaml)
        .context("Failed to write moli.yml")?;
    println!("  ✓ Updated moli.yml");

    println!("[Success] '{}' has been added to moli.yml.", selected);
    Ok(())
}

/// Determine which project the entry belongs to and compute path segments relative to that project.
fn resolve_project(
    config: &crate::project_management::config::models::MoliConfig,
    entry: &UnmanagedEntry,
) -> Result<(usize, Vec<String>)> {
    let projects = config.projects();

    // Check for root project first
    if let Some((idx, _)) = projects.iter().enumerate().find(|(_, p)| p.is_root()) {
        // Root project: path segments are the full relative path components
        let segments: Vec<String> = entry.relative_path
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(n) => Some(n.to_string_lossy().to_string()),
                _ => None,
            })
            .collect();
        return Ok((idx, segments));
    }

    // Non-root projects: check first path segment against project names
    let components: Vec<String> = entry.relative_path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(n) => Some(n.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();

    if components.is_empty() {
        bail!("Cannot resolve empty path to a project");
    }

    let first_segment = &components[0];

    // Check if first segment matches any project name
    if let Some((idx, _)) = projects.iter().enumerate().find(|(_, p)| p.name() == first_segment) {
        // Remove the project name from segments
        let segments = components[1..].to_vec();
        return Ok((idx, segments));
    }

    // No matching project found - use or create "." project
    if let Some((idx, _)) = projects.iter().enumerate().find(|(_, p)| p.name() == ".") {
        return Ok((idx, components));
    }

    // Need to create "." project - for now, bail with a message
    bail!(
        "No matching project found for '{}'. \
        Consider adding a project with `- name: .` to moli.yml, \
        or add the entry under an existing project directory.",
        entry.display_path
    );
}

/// Check if adding an entry would produce changes in moli.yml
fn would_produce_changes(
    config: &crate::project_management::config::models::MoliConfig,
    yaml_content: &str,
    entry: &UnmanagedEntry,
) -> bool {
    let (project_index, path_segments) = match resolve_project(config, entry) {
        Ok(v) => v,
        Err(_) => return true, // Keep entries that fail to resolve (let user see the error)
    };
    let language = config.projects()[project_index].language();

    let children = if entry.is_directory {
        match collect_directory_children(&entry.relative_path) {
            Ok(c) => c,
            Err(_) => return true,
        }
    } else {
        vec![]
    };

    let new_yaml = if path_segments.is_empty() && entry.is_directory {
        let mut result = yaml_content.to_string();
        for child in &children {
            let child_path = vec![child.name.clone()];
            match YamlModifier::add_entry(
                &result, project_index, &child_path,
                child.is_directory, language, &child.children,
            ) {
                Ok(r) => result = r,
                Err(_) => return true,
            }
        }
        result
    } else {
        match YamlModifier::add_entry(
            yaml_content, project_index, &path_segments,
            entry.is_directory, language, &children,
        ) {
            Ok(r) => r,
            Err(_) => return true,
        }
    };

    yaml_content != new_yaml
}

/// Collect all children of a directory as AddChild tree using ignore crate
fn collect_directory_children(dir_path: &Path) -> Result<Vec<AddChild>> {
    use ignore::WalkBuilder;

    let mut paths = Vec::new();

    let walker = WalkBuilder::new(dir_path)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for result in walker {
        let entry = result.context("Failed to read directory entry")?;
        let path = entry.path();

        // Skip the root directory itself
        if path == dir_path {
            continue;
        }

        // Skip managed/excluded files
        if let Some(file_name) = path.file_name() {
            let name = file_name.to_string_lossy();
            let skip_files = [
                "mod.rs", "__init__.py", "index.ts", "index.js",
                "Cargo.toml", "Cargo.lock", "package.json", "package-lock.json",
                "go.mod", "go.sum", ".gitignore",
            ];
            if skip_files.contains(&name.as_ref()) {
                continue;
            }
        }

        paths.push(path.to_path_buf());
    }

    Ok(AddChild::from_paths(&paths, dir_path))
}
