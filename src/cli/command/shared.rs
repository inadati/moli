use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::project_management::config::models::MoliConfig;
use crate::project_management::config::filesystem_scanner::UnmanagedEntry;
use crate::project_management::config::yaml_modifier::{YamlModifier, AddChild};

/// Determine which project the entry belongs to and compute path segments relative to that project.
pub fn resolve_project(
    config: &MoliConfig,
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
pub fn would_produce_changes(
    config: &MoliConfig,
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
pub fn collect_directory_children(dir_path: &Path) -> Result<Vec<AddChild>> {
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
