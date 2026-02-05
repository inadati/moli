use clap::Command;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use inquire::{Select, Confirm};
use std::fs;

use crate::project_management::config::ConfigParser;
use crate::project_management::config::yaml_modifier::YamlModifier;
use crate::shared::utils::diff::show_diff;

const GITHUB_ORG: &str = "itton-claude-skills";
const GITHUB_API_URL: &str = "https://api.github.com/orgs/itton-claude-skills/repos";

#[derive(Debug, Clone, Deserialize)]
struct GitHubRepo {
    name: String,
    clone_url: String,
}

pub fn spec() -> Command {
    Command::new("add")
        .about("Add Claude Code skill from itton-claude-skills organization")
        .long_about(
            "Add a Claude Code skill to moli.yml from itton-claude-skills organization.\n\
            \n\
            This command:\n\
            • Fetches available skills from GitHub\n\
            • Allows interactive selection with fuzzy search\n\
            • Creates moli.yml if it doesn't exist\n\
            • Adds the selected skill to .claude/skills directory in moli.yml\n\
            • Uses 'lang: any' with 'from' field for git clone"
        )
}

pub fn action(_sub_matches: &clap::ArgMatches) -> Result<()> {
    println!("Fetching skill list from {}...", GITHUB_ORG);

    // Fetch repository list from GitHub API
    let repos = fetch_github_repos()
        .context("Failed to fetch repository list from GitHub")?;

    if repos.is_empty() {
        bail!("No repositories found in {} organization", GITHUB_ORG);
    }

    println!("Found {} skill(s)", repos.len());

    // Read existing YAML content to check for duplicates
    let old_yaml = if ConfigParser::config_exists() {
        fs::read_to_string("moli.yml")
            .context("Failed to read moli.yml")?
    } else {
        String::new()
    };

    // Pre-filter: exclude skills that are already added
    let available_repos: Vec<GitHubRepo> = repos
        .into_iter()
        .filter(|repo| !old_yaml.contains(&format!("- from: {}", repo.clone_url)))
        .collect();

    if available_repos.is_empty() {
        println!("All available skills are already added to moli.yml.");
        return Ok(());
    }

    println!("{} skill(s) available to add", available_repos.len());

    // Select repository with inquire
    let selected_repo = select_with_inquire(&available_repos)
        .context("Selection cancelled")?;

    println!("Selected: {}", selected_repo.name);

    // Check if .claude project already exists
    let claude_project_exists = !old_yaml.is_empty() &&
        old_yaml.lines().any(|line| line.trim() == "- name: .claude");

    let new_yaml = if !claude_project_exists {
        // Add new .claude project with the skill
        let project_yaml = generate_claude_project_yaml(&selected_repo.clone_url);
        if old_yaml.is_empty() {
            println!("moli.yml not found. Creating new configuration...");
            project_yaml
        } else {
            // Use prepend_project to add .claude at the top
            YamlModifier::prepend_project(&old_yaml, &project_yaml)
                .context("Failed to add project to moli.yml")?
        }
    } else {
        // .claude project exists - add skill to existing structure
        add_skill_to_existing_claude_project(&old_yaml, &selected_repo.clone_url)?
    };

    // Show diff
    if !old_yaml.is_empty() {
        println!();
        println!("Changes to moli.yml:");
        println!("---");
        show_diff(&old_yaml, &new_yaml);
        println!("---");
    } else {
        println!();
        println!("New moli.yml content:");
        println!("---");
        println!("{}", new_yaml);
        println!("---");
    }

    // Confirm changes
    let confirm = Confirm::new("Apply changes to moli.yml?")
        .with_default(true)
        .prompt()
        .context("Confirmation cancelled")?;

    if !confirm {
        println!("moli.yml was not modified.");
        return Ok(());
    }

    // Write to moli.yml
    fs::write("moli.yml", new_yaml)
        .context("Failed to write moli.yml")?;

    println!("✓ Added {} to moli.yml", selected_repo.name);
    println!("Run 'moli up' to clone the skill repository");

    Ok(())
}

fn fetch_github_repos() -> Result<Vec<GitHubRepo>> {
    let response = ureq::get(GITHUB_API_URL)
        .call()
        .context("Failed to call GitHub API")?;

    let body = response
        .into_string()
        .context("Failed to read GitHub API response")?;

    let repos: Vec<GitHubRepo> = serde_json::from_str(&body)
        .context("Failed to parse GitHub API response")?;

    Ok(repos)
}

fn generate_claude_project_yaml(clone_url: &str) -> String {
    format!(
        r#"- name: .claude
  lang: any
  tree:
    - name: skills
      tree:
        - from: {}
"#,
        clone_url
    )
}

fn select_with_inquire(repos: &[GitHubRepo]) -> Result<GitHubRepo> {
    // Prepare display options (repo names)
    let repo_names: Vec<String> = repos.iter()
        .map(|r| r.name.clone())
        .collect();

    // Interactive selection with fuzzy search
    let selected_name = Select::new("Select a skill to add:", repo_names)
        .prompt()
        .context("Selection cancelled")?;

    // Find the selected repo
    repos.iter()
        .find(|r| r.name == selected_name)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Selected repository not found"))
}

fn add_skill_to_existing_claude_project(yaml_content: &str, clone_url: &str) -> Result<String> {
    let lines: Vec<&str> = yaml_content.lines().collect();
    let mut result_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

    // Find .claude project and skills module
    let mut in_claude_project = false;
    let mut in_skills_module = false;
    let mut in_skills_tree = false;
    let mut skills_tree_indent = 0;
    let mut insert_position = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();

        // Detect .claude project
        if trimmed == "- name: .claude" && indent == 0 {
            in_claude_project = true;
            in_skills_module = false;
            in_skills_tree = false;
            continue;
        }

        // Exit .claude project when we hit another project
        if in_claude_project && trimmed.starts_with("- name:") && indent == 0 {
            break;
        }

        // Look for skills module
        if in_claude_project && trimmed == "- name: skills" {
            in_skills_module = true;
            in_skills_tree = false;
            continue;
        }

        // Look for tree: section in skills module
        if in_skills_module && trimmed == "tree:" {
            in_skills_tree = true;
            skills_tree_indent = indent + 2;
            continue;
        }

        // Track last entry in skills tree section
        if in_skills_tree {
            if trimmed.starts_with("- from:") && indent == skills_tree_indent {
                // Found an existing skill entry, track it
                insert_position = Some(i + 1);
            } else if !trimmed.is_empty() && indent < skills_tree_indent {
                // Left the skills tree section
                break;
            }
        }
    }

    if !in_claude_project {
        bail!(".claude project not found in moli.yml");
    }

    if !in_skills_module {
        bail!("skills module not found in .claude project");
    }

    if !in_skills_tree {
        bail!("tree section not found in skills module");
    }

    // Add new skill entry
    let new_entry = format!("{}{}{}from: {}",
                           " ".repeat(skills_tree_indent),
                           "- ",
                           "",
                           clone_url);

    if let Some(pos) = insert_position {
        result_lines.insert(pos, new_entry);
    } else {
        // No existing entries, need to find where to insert after "tree:"
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if in_skills_module && trimmed == "tree:" {
                result_lines.insert(i + 1, new_entry);
                break;
            }
        }
    }

    let mut result = result_lines.join("\n");

    // Preserve trailing newline if original had one
    if yaml_content.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }

    Ok(result)
}

