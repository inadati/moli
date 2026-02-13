use clap::{Command, Arg};
use anyhow::{bail, Context, Result};
use inquire::Confirm;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::project_management::config::{ConfigParser, ConfigValidator};
use crate::project_management::config::path_collector::{PathCollector, ManagedFile};
use crate::project_management::config::filesystem_scanner::FilesystemScanner;
use crate::project_management::config::yaml_modifier::YamlModifier;
use crate::shared::utils::diff::show_diff;
use super::shared::{resolve_project, collect_directory_children, would_produce_changes};

pub fn spec() -> Command {
    Command::new("sync")
        .about("Synchronize moli.yml with the current filesystem state")
        .long_about(
            "Detect differences between moli.yml and the filesystem, then update moli.yml.\n\
            \n\
            This command will:\n\
            1. Find files/directories in moli.yml that no longer exist on disk → remove from yml\n\
            2. Find files/directories on disk not tracked in moli.yml → add to yml\n\
            3. Show a diff preview and ask for confirmation before writing\n\
            \n\
            The filesystem is the source of truth. moli.yml is updated to match.\n\
            No physical files are created or deleted by this command."
        )
        .arg(
            Arg::new("yes")
                .short('y')
                .long("yes")
                .help("Skip confirmation prompt and apply changes automatically")
                .action(clap::ArgAction::SetTrue)
        )
}

pub fn action(matches: &clap::ArgMatches) -> Result<()> {
    let auto_yes = matches.get_flag("yes");
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

    let yaml_content = fs::read_to_string("moli.yml")
        .context("Failed to read moli.yml")?;

    // === Detect entries to remove (in yml but not on FS) ===
    let managed_entries = PathCollector::collect_all_entries(&config);
    let entries_to_remove: Vec<&ManagedFile> = managed_entries
        .iter()
        .filter(|entry| {
            let path_str = if entry.is_directory {
                // display_path has trailing "/", strip it for exists() check
                entry.display_path.trim_end_matches('/').to_string()
            } else {
                entry.display_path.clone()
            };
            !Path::new(&path_str).exists()
        })
        .collect();

    // Filter out children whose parent directory is also being removed
    let entries_to_remove = filter_redundant_removals(&entries_to_remove);

    // === Detect entries to add (on FS but not in yml) ===
    let unmanaged_entries = FilesystemScanner::scan(&config)
        .context("Failed to scan filesystem")?;

    let entries_to_add: Vec<_> = unmanaged_entries
        .into_iter()
        .filter(|entry| would_produce_changes(&config, &yaml_content, entry))
        .collect();

    // === Check if there are any changes ===
    if entries_to_remove.is_empty() && entries_to_add.is_empty() {
        println!("Already in sync.");
        return Ok(());
    }

    // Show summary
    if !entries_to_remove.is_empty() {
        println!("Entries to remove from moli.yml ({}):", entries_to_remove.len());
        for entry in &entries_to_remove {
            println!("  - {}", entry.display_path);
        }
    }
    if !entries_to_add.is_empty() {
        println!("Entries to add to moli.yml ({}):", entries_to_add.len());
        for entry in &entries_to_add {
            println!("  + {}", entry.display_path);
        }
    }

    // === Apply removals first ===
    let mut yaml = yaml_content.clone();

    for entry in &entries_to_remove {
        yaml = YamlModifier::remove_entry(&yaml, entry)
            .with_context(|| format!("Failed to remove '{}' from moli.yml", entry.display_path))?;
    }

    // === Re-parse config after removals for accurate project resolution ===
    if !entries_to_add.is_empty() {
        let updated_config = ConfigParser::parse_string(&yaml)
            .context("Failed to re-parse moli.yml after removals")?;

        for entry in &entries_to_add {
            let (project_index, path_segments) = match resolve_project(&updated_config, entry) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("  ⚠ Skipping '{}': {}", entry.display_path, e);
                    continue;
                }
            };
            let language = updated_config.projects()[project_index].language();

            let children = if entry.is_directory {
                collect_directory_children(&entry.relative_path)?
            } else {
                vec![]
            };

            if path_segments.is_empty() && entry.is_directory {
                for child in &children {
                    let child_path = vec![child.name.clone()];
                    yaml = YamlModifier::add_entry(
                        &yaml,
                        project_index,
                        &child_path,
                        child.is_directory,
                        language,
                        &child.children,
                    ).context("Failed to modify moli.yml")?;
                }
            } else {
                yaml = YamlModifier::add_entry(
                    &yaml,
                    project_index,
                    &path_segments,
                    entry.is_directory,
                    language,
                    &children,
                ).context("Failed to modify moli.yml")?;
            }
        }
    }

    // === Show diff and confirm ===
    if yaml_content == yaml {
        println!("No effective changes to moli.yml.");
        return Ok(());
    }

    println!();
    println!("Changes to moli.yml:");
    println!("---");
    show_diff(&yaml_content, &yaml);
    println!("---");

    if !auto_yes {
        let confirm = Confirm::new("Apply changes to moli.yml?")
            .with_default(true)
            .prompt()
            .context("Confirmation cancelled")?;

        if !confirm {
            println!("moli.yml was not modified.");
            return Ok(());
        }
    }

    fs::write("moli.yml", &yaml)
        .context("Failed to write moli.yml")?;
    println!("  ✓ Updated moli.yml");

    println!("[Success] moli.yml has been synchronized with the filesystem.");
    Ok(())
}

/// Filter out entries whose parent directory is also being removed.
/// When YamlModifier removes a directory, all children are removed together,
/// so we only need to remove the top-level parent.
fn filter_redundant_removals<'a>(entries: &[&'a ManagedFile]) -> Vec<&'a ManagedFile> {
    let dir_paths: HashSet<&str> = entries
        .iter()
        .filter(|e| e.is_directory)
        .map(|e| e.display_path.as_str())
        .collect();

    entries
        .iter()
        .filter(|e| {
            // Keep this entry only if no parent directory is also being removed
            !dir_paths.iter().any(|dir| {
                e.display_path != *dir && e.display_path.starts_with(dir)
            })
        })
        .copied()
        .collect()
}
