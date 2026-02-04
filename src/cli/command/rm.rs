use clap::Command;
use anyhow::{bail, Context, Result};
use inquire::{Select, Confirm};
use std::fs;
use std::path::Path;

use crate::project_management::config::{ConfigParser, ConfigValidator};
use crate::project_management::config::path_collector::PathCollector;
use crate::project_management::config::yaml_modifier::YamlModifier;
use crate::shared::utils::diff::show_diff;

pub fn spec() -> Command {
    Command::new("rm")
        .about("Remove a managed file or directory from the project and moli.yml")
        .long_about(
            "Interactively select and remove a file or directory managed by moli.yml.\n\
            \n\
            This command will:\n\
            1. Parse moli.yml and list all managed files and directories\n\
            2. Let you search and select an entry to remove\n\
            3. Delete the physical file/directory (with confirmation)\n\
            4. Update moli.yml to remove the entry (with diff preview)\n\
            \n\
            Directories are shown with a trailing '/' and removing one\n\
            will also remove all files and subdirectories within it."
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

    // Collect all managed entries (files + directories)
    let managed_entries = PathCollector::collect_all_entries(&config);

    if managed_entries.is_empty() {
        println!("No managed entries found in moli.yml.");
        return Ok(());
    }

    // Build display options
    let display_options: Vec<String> = managed_entries
        .iter()
        .map(|f| f.display_path.clone())
        .collect();

    // Interactive selection with fuzzy search
    let selected = Select::new("Select a file or directory to remove:", display_options)
        .prompt()
        .context("Selection cancelled")?;

    // Find the selected entry
    let target = managed_entries
        .iter()
        .find(|f| f.display_path == selected)
        .unwrap();

    // Step 1: Physical deletion
    if target.is_directory {
        let dir_path = Path::new(&selected);
        if dir_path.exists() {
            let confirm_delete = Confirm::new(&format!("Delete directory '{}' and all its contents?", selected))
                .with_default(false)
                .prompt()
                .context("Confirmation cancelled")?;

            if !confirm_delete {
                println!("Cancelled.");
                return Ok(());
            }

            fs::remove_dir_all(dir_path)
                .with_context(|| format!("Failed to delete directory: {}", selected))?;
            println!("  ✓ Deleted {}", selected);
        } else {
            println!("  ⚠ Directory '{}' does not exist on disk (skipping physical deletion)", selected);
        }
    } else {
        let file_path = Path::new(&selected);
        if file_path.exists() {
            let confirm_delete = Confirm::new(&format!("Delete file '{}'?", selected))
                .with_default(false)
                .prompt()
                .context("Confirmation cancelled")?;

            if !confirm_delete {
                println!("Cancelled.");
                return Ok(());
            }

            fs::remove_file(file_path)
                .with_context(|| format!("Failed to delete file: {}", selected))?;
            println!("  ✓ Deleted {}", selected);
        } else {
            println!("  ⚠ File '{}' does not exist on disk (skipping physical deletion)", selected);
        }
    }

    // Step 2: Update moli.yml
    let yaml_content = fs::read_to_string("moli.yml")
        .context("Failed to read moli.yml")?;

    let new_yaml = YamlModifier::remove_entry(&yaml_content, target)
        .context("Failed to modify moli.yml")?;

    // Show diff
    println!();
    println!("Changes to moli.yml:");
    println!("---");
    show_diff(&yaml_content, &new_yaml);
    println!("---");

    let confirm_yaml = Confirm::new("Apply changes to moli.yml?")
        .with_default(true)
        .prompt()
        .context("Confirmation cancelled")?;

    if !confirm_yaml {
        println!("moli.yml was not modified.");
        return Ok(());
    }

    fs::write("moli.yml", &new_yaml)
        .context("Failed to write moli.yml")?;
    println!("  ✓ Updated moli.yml");

    println!("[Success] '{}' has been removed.", selected);
    Ok(())
}

