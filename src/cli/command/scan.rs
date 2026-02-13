use clap::Command;
use anyhow::{bail, Context, Result};
use inquire::{Select, Confirm};
use std::fs;

use crate::project_management::config::{ConfigParser, ConfigValidator};
use crate::project_management::config::filesystem_scanner::{FilesystemScanner, UnmanagedEntry};
use crate::project_management::config::yaml_modifier::YamlModifier;
use crate::shared::utils::diff::show_diff;
use super::shared::{resolve_project, collect_directory_children, would_produce_changes};

pub fn spec() -> Command {
    Command::new("scan")
        .about("Scan and import unmanaged files or directories into moli.yml")
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

