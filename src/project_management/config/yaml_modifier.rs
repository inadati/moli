use std::collections::BTreeMap;
use std::path::Path;
use anyhow::{Result, bail};
use crate::project_management::config::path_collector::ManagedFile;
use crate::project_management::config::filesystem_scanner::FilesystemScanner;

pub struct YamlModifier;

impl YamlModifier {
    /// Remove a file or directory entry from YAML content (string-based editing to preserve formatting)
    pub fn remove_entry(yaml_content: &str, target: &ManagedFile) -> Result<String> {
        if target.is_directory {
            Self::remove_module_entry(yaml_content, target)
        } else {
            Self::remove_file_entry(yaml_content, target)
        }
    }

    /// Remove a file entry from YAML content
    pub fn remove_file_entry(yaml_content: &str, target: &ManagedFile) -> Result<String> {
        let lines: Vec<&str> = yaml_content.lines().collect();
        let target_module_path = &target.module_path;

        // Find the target file's `- name: xxx` line
        let removal_range = if target.is_project_level {
            Self::find_project_level_file(&lines, target)?
        } else {
            Self::find_tree_file(&lines, target_module_path, &target.file_name)?
        };

        let (start, end) = match removal_range {
            Some(range) => range,
            None => bail!("Could not find file entry '{}' in moli.yml", target.file_name),
        };

        // Build result by removing the target lines
        let mut result_lines: Vec<&str> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if i >= start && i <= end {
                continue;
            }
            result_lines.push(line);
        }

        // Check if the file: section is now empty and remove it if so
        let mut result = result_lines.join("\n");
        result = Self::remove_empty_file_sections(&result);

        // Preserve trailing newline if original had one
        if yaml_content.ends_with('\n') && !result.ends_with('\n') {
            result.push('\n');
        }

        Ok(result)
    }

    /// Remove a module (directory) entry from YAML content, including all its children
    pub fn remove_module_entry(yaml_content: &str, target: &ManagedFile) -> Result<String> {
        let lines: Vec<&str> = yaml_content.lines().collect();

        // Navigate to the parent context, then find the `- name: <module_name>` entry
        let module_name = &target.file_name;
        let parent_path = &target.module_path;

        // Find the module's `- name: xxx` line
        let module_start = if parent_path.is_empty() {
            // Top-level tree entry: find within the correct project's tree section
            Self::find_top_level_module(&lines, target.project_index, module_name)?
        } else {
            // Nested module: navigate parent path first
            let mut search_start = 0;
            for parent_name in parent_path {
                match Self::find_module_start(&lines, search_start, parent_name)? {
                    Some(idx) => search_start = idx + 1,
                    None => bail!("Could not find parent module '{}' in moli.yml", parent_name),
                }
            }
            Self::find_module_start(&lines, search_start, module_name)?
        };

        let module_start = match module_start {
            Some(idx) => idx,
            None => bail!("Could not find module '{}' in moli.yml", module_name),
        };

        let module_indent = Self::line_indent(lines[module_start]);

        // Find the end of this module (all lines with greater indent, or empty lines between them)
        let mut module_end = module_start;
        for i in (module_start + 1)..lines.len() {
            let trimmed = lines[i].trim();
            let indent = Self::line_indent(lines[i]);

            if trimmed.is_empty() {
                // Empty line might be inside the module block; check next non-empty line
                continue;
            }

            if indent > module_indent {
                module_end = i;
            } else {
                // We've hit a line at or above the module's indent - stop
                break;
            }
        }

        // Build result by removing the module lines
        let mut result_lines: Vec<&str> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if i >= module_start && i <= module_end {
                continue;
            }
            result_lines.push(line);
        }

        // Clean up empty tree: sections
        let mut result = result_lines.join("\n");
        result = Self::remove_empty_tree_sections(&result);

        // Preserve trailing newline if original had one
        if yaml_content.ends_with('\n') && !result.ends_with('\n') {
            result.push('\n');
        }

        Ok(result)
    }

    /// Find a top-level module within a specific project's tree section
    fn find_top_level_module(
        lines: &[&str],
        project_index: usize,
        module_name: &str,
    ) -> Result<Option<usize>> {
        let mut current_project_index: i32 = -1;
        let mut in_tree_section = false;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("- name:") && Self::line_indent(line) == 0 {
                current_project_index += 1;
                in_tree_section = false;
            }

            if current_project_index as usize != project_index {
                continue;
            }

            if trimmed == "tree:" && Self::line_indent(line) == 2 {
                in_tree_section = true;
                continue;
            }

            if in_tree_section {
                let expected = format!("- name: {}", module_name);
                if trimmed == expected && Self::line_indent(line) == 4 {
                    return Ok(Some(i));
                }
            }
        }

        Ok(None)
    }

    /// Find a project-level file entry (directly under a project, not in tree)
    fn find_project_level_file(
        lines: &[&str],
        target: &ManagedFile,
    ) -> Result<Option<(usize, usize)>> {
        let mut current_project_index: i32 = -1;
        let mut in_project_file_section = false;
        let mut project_indent = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("- name:") && Self::line_indent(line) == 0 {
                current_project_index += 1;
                in_project_file_section = false;
                project_indent = 2;
            }

            if current_project_index as usize != target.project_index {
                continue;
            }

            if trimmed == "file:" && Self::line_indent(line) == project_indent {
                in_project_file_section = true;
                continue;
            }

            if in_project_file_section && Self::line_indent(line) <= project_indent && !trimmed.is_empty() {
                if !trimmed.starts_with("- name:") && !trimmed.starts_with("pub:") {
                    in_project_file_section = false;
                }
            }

            if in_project_file_section {
                if let Some(range) = Self::match_file_entry(lines, i, &target.file_name, project_indent + 2)? {
                    return Ok(Some(range));
                }
            }
        }

        Ok(None)
    }

    /// Find a file entry inside the tree structure
    fn find_tree_file(
        lines: &[&str],
        module_path: &[String],
        file_name: &str,
    ) -> Result<Option<(usize, usize)>> {
        let mut search_start = 0;

        for module_name in module_path {
            match Self::find_module_start(lines, search_start, module_name)? {
                Some(idx) => search_start = idx + 1,
                None => return Ok(None),
            }
        }

        let last_module = module_path.last().unwrap();
        let module_start = Self::find_module_start(lines, search_start - 1, last_module)?;
        if module_start.is_none() {
            return Ok(None);
        }
        let module_start = module_start.unwrap();
        let module_indent = Self::line_indent(lines[module_start]);

        let file_section_indent = module_indent + 2;
        let mut in_file_section = false;

        for i in (module_start + 1)..lines.len() {
            let line = lines[i];
            let trimmed = line.trim();
            let indent = Self::line_indent(line);

            if !trimmed.is_empty() && indent <= module_indent {
                break;
            }

            if trimmed == "file:" && indent == file_section_indent {
                in_file_section = true;
                continue;
            }

            if in_file_section && indent == file_section_indent && !trimmed.is_empty() {
                if !trimmed.starts_with("- name:") && !trimmed.starts_with("pub:") {
                    in_file_section = false;
                }
            }

            if in_file_section {
                if let Some(range) = Self::match_file_entry(lines, i, file_name, file_section_indent + 2)? {
                    return Ok(Some(range));
                }
            }
        }

        Ok(None)
    }

    /// Find the start line of a module (- name: xxx) from a given starting line
    fn find_module_start(lines: &[&str], from: usize, module_name: &str) -> Result<Option<usize>> {
        for i in from..lines.len() {
            let trimmed = lines[i].trim();
            let expected = format!("- name: {}", module_name);
            if trimmed == expected {
                return Ok(Some(i));
            }
        }
        Ok(None)
    }

    /// Try to match a file entry at the given line, return the range of lines to remove
    fn match_file_entry(
        lines: &[&str],
        line_index: usize,
        file_name: &str,
        expected_indent: usize,
    ) -> Result<Option<(usize, usize)>> {
        let line = lines[line_index];
        let trimmed = line.trim();
        let indent = Self::line_indent(line);

        if indent != expected_indent {
            return Ok(None);
        }

        let expected = format!("- name: {}", file_name);
        if trimmed != expected {
            return Ok(None);
        }

        let mut end = line_index;
        for j in (line_index + 1)..lines.len() {
            let next_trimmed = lines[j].trim();
            let next_indent = Self::line_indent(lines[j]);

            if next_trimmed.is_empty() {
                break;
            }

            if next_indent > expected_indent && !next_trimmed.starts_with("- name:") {
                end = j;
            } else {
                break;
            }
        }

        Ok(Some((line_index, end)))
    }

    /// Calculate the indentation level of a line
    fn line_indent(line: &str) -> usize {
        line.len() - line.trim_start().len()
    }

    /// Remove `file:` sections that have become empty after file removal
    fn remove_empty_file_sections(content: &str) -> String {
        Self::remove_empty_sections(content, "file:")
    }

    /// Remove `tree:` sections that have become empty after module removal
    fn remove_empty_tree_sections(content: &str) -> String {
        Self::remove_empty_sections(content, "tree:")
    }

    /// Remove sections of a given key that have become empty
    fn remove_empty_sections(content: &str, section_key: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut result: Vec<&str> = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            let trimmed = lines[i].trim();

            if trimmed == section_key {
                let indent = Self::line_indent(lines[i]);
                let mut has_entries = false;
                for j in (i + 1)..lines.len() {
                    let next_trimmed = lines[j].trim();
                    if next_trimmed.is_empty() {
                        continue;
                    }
                    let next_indent = Self::line_indent(lines[j]);
                    if next_indent > indent && next_trimmed.starts_with("- name:") {
                        has_entries = true;
                    }
                    break;
                }

                if !has_entries {
                    i += 1;
                    continue;
                }
            }

            result.push(lines[i]);
            i += 1;
        }

        result.join("\n")
    }

    // ========== Add Entry Logic ==========

    /// Add a file or directory entry to YAML content at the appropriate position in a project.
    /// `path_segments` is the path components relative to the project root (e.g., ["src", "domain", "model.rs"]).
    /// `is_directory` indicates whether the entry itself is a directory.
    /// `language` is the project language for extension stripping.
    /// `project_index` is which project in the YAML to modify.
    /// `children` contains child entries when adding a directory (sub-dirs and files found inside).
    pub fn add_entry(
        yaml_content: &str,
        project_index: usize,
        path_segments: &[String],
        is_directory: bool,
        language: &str,
        children: &[AddChild],
    ) -> Result<String> {
        if path_segments.is_empty() {
            bail!("Cannot add entry with empty path");
        }

        let mut result = yaml_content.to_string();

        if is_directory {
            // Adding a directory: need to add as tree entry, then recursively add children
            result = Self::ensure_tree_path(&result, project_index, path_segments)?;

            // Add children (files and subdirectories)
            for child in children {
                result = Self::add_child_recursive(&result, project_index, path_segments, child, language)?;
            }
        } else {
            // Adding a single file
            let dir_path = &path_segments[..path_segments.len() - 1];
            let file_name = &path_segments[path_segments.len() - 1];
            let moli_name = FilesystemScanner::filename_without_standard_extension(file_name, language);

            if dir_path.is_empty() {
                // Project-level file
                result = Self::add_project_level_file(&result, project_index, &moli_name)?;
            } else {
                // File inside tree
                result = Self::ensure_tree_path(&result, project_index, dir_path)?;
                result = Self::add_file_to_module(&result, project_index, dir_path, &moli_name)?;
            }
        }

        // Preserve trailing newline
        if yaml_content.ends_with('\n') && !result.ends_with('\n') {
            result.push('\n');
        }

        Ok(result)
    }

    /// Recursively add children (files/subdirs) under a parent directory path
    fn add_child_recursive(
        yaml_content: &str,
        project_index: usize,
        parent_segments: &[String],
        child: &AddChild,
        language: &str,
    ) -> Result<String> {
        let mut full_path = parent_segments.to_vec();
        full_path.push(child.name.clone());

        if child.is_directory {
            let mut result = Self::ensure_tree_path(yaml_content, project_index, &full_path)?;
            for sub_child in &child.children {
                result = Self::add_child_recursive(&result, project_index, &full_path, sub_child, language)?;
            }
            Ok(result)
        } else {
            let moli_name = FilesystemScanner::filename_without_standard_extension(&child.name, language);
            Self::add_file_to_module(yaml_content, project_index, parent_segments, &moli_name)
        }
    }

    /// Ensure a tree path exists in the YAML (create tree entries as needed).
    /// `segments` are the directory names (e.g., ["src", "domain"]).
    fn ensure_tree_path(
        yaml_content: &str,
        project_index: usize,
        segments: &[String],
    ) -> Result<String> {
        let mut result = yaml_content.to_string();

        for depth in 0..segments.len() {
            let current_segments = &segments[..=depth];
            if !Self::module_exists(&result, project_index, current_segments) {
                let parent = &current_segments[..current_segments.len() - 1];
                let module_name = &current_segments[current_segments.len() - 1];
                result = Self::add_module(&result, project_index, parent, module_name)?;
            }
        }

        Ok(result)
    }

    /// Check if a module path already exists in the YAML
    fn module_exists(yaml_content: &str, project_index: usize, segments: &[String]) -> bool {
        let lines: Vec<&str> = yaml_content.lines().collect();

        if segments.is_empty() {
            return true;
        }

        if segments.len() == 1 {
            // Top-level module
            return Self::find_top_level_module(&lines, project_index, &segments[0])
                .ok()
                .flatten()
                .is_some();
        }

        // Navigate to parent first
        let mut search_start = 0;
        for seg in &segments[..segments.len() - 1] {
            match Self::find_module_start(&lines, search_start, seg) {
                Ok(Some(idx)) => search_start = idx + 1,
                _ => return false,
            }
        }

        Self::find_module_start(&lines, search_start, segments.last().unwrap())
            .ok()
            .flatten()
            .is_some()
    }

    /// Add a module (directory) entry under a parent path
    fn add_module(
        yaml_content: &str,
        project_index: usize,
        parent_segments: &[String],
        module_name: &str,
    ) -> Result<String> {
        let lines: Vec<&str> = yaml_content.lines().collect();

        if parent_segments.is_empty() {
            // Add as top-level tree entry in project
            Self::add_top_level_module(&lines, project_index, module_name)
        } else {
            // Add as nested tree entry
            Self::add_nested_module(&lines, project_index, parent_segments, module_name)
        }
    }

    /// Add a top-level module to a project's tree section
    fn add_top_level_module(
        lines: &[&str],
        project_index: usize,
        module_name: &str,
    ) -> Result<String> {
        let mut result_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        let mut current_project: i32 = -1;
        let mut tree_line = None;
        let mut insert_pos = None;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("- name:") && Self::line_indent(line) == 0 {
                current_project += 1;
                if current_project as usize > project_index {
                    // We've passed our project, insert before this project
                    insert_pos = Some(i);
                    break;
                }
            }

            if current_project as usize == project_index {
                if trimmed == "tree:" && Self::line_indent(line) == 2 {
                    tree_line = Some(i);
                }
                // Find the last entry in this project's tree section
                if tree_line.is_some() && Self::line_indent(line) == 4 && trimmed.starts_with("- name:") {
                    // Track last tree entry
                    // Find end of this module block
                    let mut end = i;
                    for j in (i + 1)..lines.len() {
                        let next_trimmed = lines[j].trim();
                        let next_indent = Self::line_indent(lines[j]);
                        if next_trimmed.is_empty() {
                            continue;
                        }
                        if next_indent > 4 {
                            end = j;
                        } else {
                            break;
                        }
                    }
                    insert_pos = Some(end + 1);
                }
            }
        }

        if tree_line.is_none() {
            // No tree: section exists, create one at the end of the project
            let project_end = Self::find_project_end(lines, project_index);
            let new_lines = format!("  tree:\n    - name: {}", module_name);
            result_lines.insert(project_end, new_lines);
        } else if let Some(pos) = insert_pos {
            let new_line = format!("    - name: {}", module_name);
            result_lines.insert(pos, new_line);
        }

        Ok(result_lines.join("\n"))
    }

    /// Add a nested module under a parent module
    fn add_nested_module(
        lines: &[&str],
        project_index: usize,
        parent_segments: &[String],
        module_name: &str,
    ) -> Result<String> {
        let mut result_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

        // Find the parent module
        let mut search_start = 0;
        // First, navigate to the correct project
        let mut current_project: i32 = -1;
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("- name:") && Self::line_indent(line) == 0 {
                current_project += 1;
                if current_project as usize == project_index {
                    search_start = i;
                    break;
                }
            }
        }

        for seg in parent_segments {
            match Self::find_module_start(lines, search_start, seg)? {
                Some(idx) => search_start = idx + 1,
                None => bail!("Could not find parent module '{}' in moli.yml", seg),
            }
        }

        // search_start - 1 is the parent module's `- name:` line
        let parent_start = search_start - 1;
        let parent_indent = Self::line_indent(lines[parent_start]);
        let tree_indent = parent_indent + 2;
        let entry_indent = parent_indent + 4;

        // Look for an existing tree: section in this parent
        let mut tree_section_found = false;
        let mut insert_pos = None;

        for i in (parent_start + 1)..lines.len() {
            let trimmed = lines[i].trim();
            let indent = Self::line_indent(lines[i]);

            if !trimmed.is_empty() && indent <= parent_indent {
                break; // Left the parent module
            }

            if trimmed == "tree:" && indent == tree_indent {
                tree_section_found = true;
                continue;
            }

            if tree_section_found {
                if indent == entry_indent && trimmed.starts_with("- name:") {
                    // Track to find last entry in tree section
                    let mut end = i;
                    for j in (i + 1)..lines.len() {
                        let next_trimmed = lines[j].trim();
                        let next_indent = Self::line_indent(lines[j]);
                        if next_trimmed.is_empty() {
                            continue;
                        }
                        if next_indent > entry_indent {
                            end = j;
                        } else {
                            break;
                        }
                    }
                    insert_pos = Some(end + 1);
                }
            }
        }

        if !tree_section_found {
            // Need to create tree: section at end of parent module
            let mut parent_end = parent_start;
            for i in (parent_start + 1)..lines.len() {
                let trimmed = lines[i].trim();
                let indent = Self::line_indent(lines[i]);
                if !trimmed.is_empty() && indent <= parent_indent {
                    break;
                }
                if !trimmed.is_empty() {
                    parent_end = i;
                }
            }
            let tree_line = format!("{}tree:", " ".repeat(tree_indent));
            let entry_line = format!("{}- name: {}", " ".repeat(entry_indent), module_name);
            result_lines.insert(parent_end + 1, tree_line);
            result_lines.insert(parent_end + 2, entry_line);
        } else if let Some(pos) = insert_pos {
            let new_line = format!("{}- name: {}", " ".repeat(entry_indent), module_name);
            result_lines.insert(pos, new_line);
        } else {
            // tree: section exists but is empty (shouldn't happen with valid data)
            // Find tree: line and insert after it
            for i in (parent_start + 1)..lines.len() {
                let trimmed = lines[i].trim();
                let indent = Self::line_indent(lines[i]);
                if trimmed == "tree:" && indent == tree_indent {
                    let new_line = format!("{}- name: {}", " ".repeat(entry_indent), module_name);
                    result_lines.insert(i + 1, new_line);
                    break;
                }
            }
        }

        Ok(result_lines.join("\n"))
    }

    /// Add a file to a module's file section
    fn add_file_to_module(
        yaml_content: &str,
        project_index: usize,
        module_segments: &[String],
        file_name: &str,
    ) -> Result<String> {
        let lines: Vec<&str> = yaml_content.lines().collect();
        let mut result_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

        // Find the target module
        let mut search_start = 0;
        let mut current_project: i32 = -1;
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("- name:") && Self::line_indent(line) == 0 {
                current_project += 1;
                if current_project as usize == project_index {
                    search_start = i;
                    break;
                }
            }
        }

        for seg in module_segments {
            match Self::find_module_start(&lines, search_start, seg)? {
                Some(idx) => search_start = idx + 1,
                None => bail!("Could not find module '{}' in moli.yml", seg),
            }
        }

        let module_start = search_start - 1;
        let module_indent = Self::line_indent(lines[module_start]);
        let file_section_indent = module_indent + 2;
        let file_entry_indent = module_indent + 4;

        // Check if file already exists
        let mut in_file_section = false;
        for i in (module_start + 1)..lines.len() {
            let trimmed = lines[i].trim();
            let indent = Self::line_indent(lines[i]);

            if !trimmed.is_empty() && indent <= module_indent {
                break;
            }

            if trimmed == "file:" && indent == file_section_indent {
                in_file_section = true;
                continue;
            }

            if in_file_section && indent == file_entry_indent {
                let expected = format!("- name: {}", file_name);
                if trimmed == expected {
                    // File already exists, skip
                    return Ok(yaml_content.to_string());
                }
            }
        }

        // Find or create file: section and add entry
        let mut file_section_found = false;
        let mut insert_pos = None;

        for i in (module_start + 1)..lines.len() {
            let trimmed = lines[i].trim();
            let indent = Self::line_indent(lines[i]);

            if !trimmed.is_empty() && indent <= module_indent {
                break;
            }

            if trimmed == "file:" && indent == file_section_indent {
                file_section_found = true;
                insert_pos = Some(i + 1); // Default: right after file:
                continue;
            }

            if file_section_found && indent == file_entry_indent && trimmed.starts_with("- name:") {
                // Track to find last file entry (including any sub-properties like pub:)
                let mut end = i;
                for j in (i + 1)..lines.len() {
                    let next_trimmed = lines[j].trim();
                    let next_indent = Self::line_indent(lines[j]);
                    if next_trimmed.is_empty() {
                        break;
                    }
                    if next_indent > file_entry_indent && !next_trimmed.starts_with("- name:") {
                        end = j;
                    } else {
                        break;
                    }
                }
                insert_pos = Some(end + 1);
            }

            // If we hit tree: at the same indent, stop looking for file entries
            if file_section_found && trimmed == "tree:" && indent == file_section_indent {
                break;
            }
        }

        if !file_section_found {
            // Need to create file: section. Insert before tree: section if it exists, or at end of module.
            let mut tree_pos = None;
            let mut module_content_end = module_start;

            for i in (module_start + 1)..lines.len() {
                let trimmed = lines[i].trim();
                let indent = Self::line_indent(lines[i]);

                if !trimmed.is_empty() && indent <= module_indent {
                    break;
                }

                if !trimmed.is_empty() {
                    module_content_end = i;
                }

                if trimmed == "tree:" && indent == file_section_indent {
                    tree_pos = Some(i);
                    break;
                }
            }

            if let Some(tp) = tree_pos {
                // Insert file: section before tree:
                let file_line = format!("{}file:", " ".repeat(file_section_indent));
                let entry_line = format!("{}- name: {}", " ".repeat(file_entry_indent), file_name);
                result_lines.insert(tp, entry_line);
                result_lines.insert(tp, file_line);
            } else {
                // Insert at end of module
                let file_line = format!("{}file:", " ".repeat(file_section_indent));
                let entry_line = format!("{}- name: {}", " ".repeat(file_entry_indent), file_name);
                result_lines.insert(module_content_end + 1, file_line);
                result_lines.insert(module_content_end + 2, entry_line);
            }
        } else if let Some(pos) = insert_pos {
            let new_line = format!("{}- name: {}", " ".repeat(file_entry_indent), file_name);
            result_lines.insert(pos, new_line);
        }

        Ok(result_lines.join("\n"))
    }

    /// Add a project-level file (directly under the project, not in tree)
    fn add_project_level_file(
        yaml_content: &str,
        project_index: usize,
        file_name: &str,
    ) -> Result<String> {
        let lines: Vec<&str> = yaml_content.lines().collect();
        let mut result_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

        let mut current_project: i32 = -1;
        let mut file_section_found = false;
        let mut insert_pos = None;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("- name:") && Self::line_indent(line) == 0 {
                current_project += 1;
                file_section_found = false;
            }

            if current_project as usize != project_index {
                continue;
            }

            if trimmed == "file:" && Self::line_indent(line) == 2 {
                file_section_found = true;
                insert_pos = Some(i + 1);
                continue;
            }

            if file_section_found && Self::line_indent(line) == 4 && trimmed.starts_with("- name:") {
                // Check for duplicate
                let expected = format!("- name: {}", file_name);
                if trimmed == expected {
                    return Ok(yaml_content.to_string());
                }
                let mut end = i;
                for j in (i + 1)..lines.len() {
                    let next_trimmed = lines[j].trim();
                    let next_indent = Self::line_indent(lines[j]);
                    if next_trimmed.is_empty() {
                        break;
                    }
                    if next_indent > 4 && !next_trimmed.starts_with("- name:") {
                        end = j;
                    } else {
                        break;
                    }
                }
                insert_pos = Some(end + 1);
            }
        }

        if !file_section_found {
            // Add file: section before tree: or at end of project
            let mut tree_pos = None;
            current_project = -1;

            for (i, line) in lines.iter().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("- name:") && Self::line_indent(line) == 0 {
                    current_project += 1;
                }
                if current_project as usize != project_index {
                    continue;
                }
                if trimmed == "tree:" && Self::line_indent(line) == 2 {
                    tree_pos = Some(i);
                    break;
                }
            }

            if let Some(tp) = tree_pos {
                result_lines.insert(tp, format!("    - name: {}", file_name));
                result_lines.insert(tp, "  file:".to_string());
            } else {
                let project_end = Self::find_project_end(&lines, project_index);
                result_lines.insert(project_end, format!("    - name: {}", file_name));
                result_lines.insert(project_end, "  file:".to_string());
            }
        } else if let Some(pos) = insert_pos {
            let new_line = format!("    - name: {}", file_name);
            result_lines.insert(pos, new_line);
        }

        Ok(result_lines.join("\n"))
    }

    /// Find the end line index of a project (exclusive - line after last content)
    fn find_project_end(lines: &[&str], project_index: usize) -> usize {
        let mut current_project: i32 = -1;
        let mut project_start = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("- name:") && Self::line_indent(line) == 0 {
                current_project += 1;
                if current_project as usize == project_index {
                    project_start = i;
                }
                if current_project as usize > project_index {
                    return i;
                }
            }
        }

        // Last project - return end of file
        let _ = project_start;
        lines.len()
    }
}

/// Represents a child entry to be added (file or subdirectory with its own children)
#[derive(Debug, Clone)]
pub struct AddChild {
    pub name: String,
    pub is_directory: bool,
    pub children: Vec<AddChild>,
}

impl AddChild {
    /// Build a tree of AddChild from a list of relative file paths under a base directory
    pub fn from_paths(paths: &[std::path::PathBuf], base: &std::path::Path) -> Vec<AddChild> {
        let mut tree: BTreeMap<String, Vec<std::path::PathBuf>> = BTreeMap::new();
        let mut files: Vec<String> = Vec::new();

        for path in paths {
            let relative = match path.strip_prefix(base) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let components: Vec<&std::ffi::OsStr> = relative.components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(n) => Some(n),
                    _ => None,
                })
                .collect();

            if components.is_empty() {
                continue;
            }

            if components.len() == 1 {
                let name = components[0].to_string_lossy().to_string();
                if path.is_dir() {
                    tree.entry(name).or_default();
                } else {
                    files.push(name);
                }
            } else {
                let dir_name = components[0].to_string_lossy().to_string();
                tree.entry(dir_name.clone()).or_default();
                tree.get_mut(&dir_name).unwrap().push(
                    components[1..].iter().collect::<std::path::PathBuf>()
                );
            }
        }

        let mut result = Vec::new();

        // Add directories first
        for (dir_name, sub_paths) in &tree {
            let sub_base = Path::new("");
            let children = Self::from_paths(sub_paths, sub_base);
            result.push(AddChild {
                name: dir_name.clone(),
                is_directory: true,
                children,
            });
        }

        // Add files
        for file_name in &files {
            result.push(AddChild {
                name: file_name.clone(),
                is_directory: false,
                children: vec![],
            });
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_file_target(
        file_name: &str,
        module_path: Vec<String>,
        project_index: usize,
        is_project_level: bool,
    ) -> ManagedFile {
        ManagedFile {
            display_path: String::new(),
            project_index,
            file_name: file_name.to_string(),
            module_path,
            is_project_level,
            is_directory: false,
        }
    }

    fn make_dir_target(
        module_name: &str,
        parent_path: Vec<String>,
        project_index: usize,
    ) -> ManagedFile {
        ManagedFile {
            display_path: String::new(),
            project_index,
            file_name: module_name.to_string(),
            module_path: parent_path,
            is_project_level: false,
            is_directory: true,
        }
    }

    #[test]
    fn test_remove_file_from_tree() {
        let yaml = "\
- name: app
  root: true
  lang: rust
  tree:
    - name: src
      file:
        - name: main
      tree:
        - name: domain
          file:
            - name: model
            - name: repository
";

        let target = make_file_target(
            "model",
            vec!["src".to_string(), "domain".to_string()],
            0,
            false,
        );

        let result = YamlModifier::remove_entry(yaml, &target).unwrap();

        assert!(result.contains("- name: repository"));
        assert!(!result.contains("- name: model"));
        assert!(result.contains("file:"));
    }

    #[test]
    fn test_remove_last_file_removes_file_section() {
        let yaml = "\
- name: app
  root: true
  lang: rust
  tree:
    - name: src
      file:
        - name: main
      tree:
        - name: domain
          file:
            - name: model
";

        let target = make_file_target(
            "model",
            vec!["src".to_string(), "domain".to_string()],
            0,
            false,
        );

        let result = YamlModifier::remove_entry(yaml, &target).unwrap();

        let domain_section: String = result.lines()
            .skip_while(|l| !l.contains("- name: domain"))
            .take(3)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!domain_section.contains("file:"));
    }

    #[test]
    fn test_remove_project_level_file() {
        let yaml = "\
- name: docs
  root: true
  lang: any
  file:
    - name: README.md
    - name: CHANGELOG.md
";

        let target = make_file_target("README.md", vec![], 0, true);

        let result = YamlModifier::remove_entry(yaml, &target).unwrap();

        assert!(!result.contains("README.md"));
        assert!(result.contains("CHANGELOG.md"));
    }

    #[test]
    fn test_remove_file_with_pub_attribute() {
        let yaml = "\
- name: app
  root: true
  lang: rust
  tree:
    - name: src
      tree:
        - name: domain
          file:
            - name: model
              pub: crate
            - name: repository
";

        let target = make_file_target(
            "model",
            vec!["src".to_string(), "domain".to_string()],
            0,
            false,
        );

        let result = YamlModifier::remove_entry(yaml, &target).unwrap();

        assert!(!result.contains("- name: model"));
        assert!(!result.contains("pub: crate"));
        assert!(result.contains("- name: repository"));
    }

    #[test]
    fn test_preserves_trailing_newline() {
        let yaml = "\
- name: app
  root: true
  lang: rust
  tree:
    - name: src
      file:
        - name: main
        - name: lib
";

        let target = make_file_target(
            "lib",
            vec!["src".to_string()],
            0,
            false,
        );

        let result = YamlModifier::remove_entry(yaml, &target).unwrap();
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn test_remove_module_with_files() {
        let yaml = "\
- name: app
  root: true
  lang: rust
  tree:
    - name: src
      file:
        - name: main
      tree:
        - name: domain
          file:
            - name: model
            - name: repository
        - name: api
          file:
            - name: handler
";

        let target = make_dir_target("domain", vec!["src".to_string()], 0);

        let result = YamlModifier::remove_entry(yaml, &target).unwrap();

        assert!(!result.contains("domain"));
        assert!(!result.contains("model"));
        assert!(!result.contains("repository"));
        assert!(result.contains("- name: api"));
        assert!(result.contains("- name: handler"));
        assert!(result.contains("- name: main"));
    }

    #[test]
    fn test_remove_module_with_subtree() {
        let yaml = "\
- name: app
  root: true
  lang: rust
  tree:
    - name: src
      file:
        - name: main
      tree:
        - name: domain
          file:
            - name: model
          tree:
            - name: entity
              file:
                - name: user
                - name: order
        - name: api
          file:
            - name: handler
";

        let target = make_dir_target("domain", vec!["src".to_string()], 0);

        let result = YamlModifier::remove_entry(yaml, &target).unwrap();

        assert!(!result.contains("domain"));
        assert!(!result.contains("model"));
        assert!(!result.contains("entity"));
        assert!(!result.contains("user"));
        assert!(!result.contains("order"));
        assert!(result.contains("- name: api"));
        assert!(result.contains("- name: handler"));
    }

    #[test]
    fn test_remove_last_module_removes_tree_section() {
        let yaml = "\
- name: app
  root: true
  lang: rust
  tree:
    - name: src
      file:
        - name: main
      tree:
        - name: domain
          file:
            - name: model
";

        let target = make_dir_target("domain", vec!["src".to_string()], 0);

        let result = YamlModifier::remove_entry(yaml, &target).unwrap();

        assert!(!result.contains("domain"));
        assert!(!result.contains("model"));
        // The tree: section under src should be removed since it's now empty
        let src_section: String = result.lines()
            .skip_while(|l| !l.contains("- name: src"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!src_section.contains("tree:"));
    }

    #[test]
    fn test_remove_top_level_module() {
        let yaml = "\
- name: app
  root: true
  lang: rust
  tree:
    - name: src
      file:
        - name: main
    - name: tests
      file:
        - name: integration
";

        let target = make_dir_target("tests", vec![], 0);

        let result = YamlModifier::remove_entry(yaml, &target).unwrap();

        assert!(!result.contains("tests"));
        assert!(!result.contains("integration"));
        assert!(result.contains("- name: src"));
        assert!(result.contains("- name: main"));
    }

    // ========== Add Entry Tests ==========

    #[test]
    fn test_add_file_to_existing_module() {
        let yaml = "\
- name: app
  root: true
  lang: rust
  tree:
    - name: src
      file:
        - name: main
      tree:
        - name: domain
          file:
            - name: model
";

        let result = YamlModifier::add_entry(
            yaml,
            0,
            &["src".to_string(), "domain".to_string(), "repository.rs".to_string()],
            false,
            "rust",
            &[],
        ).unwrap();

        assert!(result.contains("- name: repository"));
        assert!(result.contains("- name: model"));
    }

    #[test]
    fn test_add_file_creates_file_section() {
        let yaml = "\
- name: app
  root: true
  lang: rust
  tree:
    - name: src
      tree:
        - name: domain
";

        let result = YamlModifier::add_entry(
            yaml,
            0,
            &["src".to_string(), "domain".to_string(), "model.rs".to_string()],
            false,
            "rust",
            &[],
        ).unwrap();

        assert!(result.contains("file:"));
        assert!(result.contains("- name: model"));
    }

    #[test]
    fn test_add_new_module() {
        let yaml = "\
- name: app
  root: true
  lang: rust
  tree:
    - name: src
      file:
        - name: main
";

        let result = YamlModifier::add_entry(
            yaml,
            0,
            &["src".to_string(), "api".to_string()],
            true,
            "rust",
            &[],
        ).unwrap();

        assert!(result.contains("- name: api"));
        assert!(result.contains("- name: src"));
    }

    #[test]
    fn test_add_directory_with_children() {
        let yaml = "\
- name: app
  root: true
  lang: rust
  tree:
    - name: src
      file:
        - name: main
";

        let children = vec![
            AddChild {
                name: "handler.rs".to_string(),
                is_directory: false,
                children: vec![],
            },
            AddChild {
                name: "router.rs".to_string(),
                is_directory: false,
                children: vec![],
            },
        ];

        let result = YamlModifier::add_entry(
            yaml,
            0,
            &["src".to_string(), "api".to_string()],
            true,
            "rust",
            &children,
        ).unwrap();

        assert!(result.contains("- name: api"));
        assert!(result.contains("- name: handler"));
        assert!(result.contains("- name: router"));
    }

    #[test]
    fn test_add_project_level_file() {
        let yaml = "\
- name: docs
  root: true
  lang: any
  tree:
    - name: src
";

        let result = YamlModifier::add_entry(
            yaml,
            0,
            &["README.md".to_string()],
            false,
            "any",
            &[],
        ).unwrap();

        assert!(result.contains("file:"));
        assert!(result.contains("- name: README.md"));
    }

    #[test]
    fn test_add_duplicate_file_is_noop() {
        let yaml = "\
- name: app
  root: true
  lang: rust
  tree:
    - name: src
      file:
        - name: main
";

        let result = YamlModifier::add_entry(
            yaml,
            0,
            &["src".to_string(), "main.rs".to_string()],
            false,
            "rust",
            &[],
        ).unwrap();

        // Should be unchanged
        assert_eq!(result, yaml);
    }

    #[test]
    fn test_add_top_level_module_creates_tree() {
        let yaml = "\
- name: app
  root: true
  lang: rust
";

        let result = YamlModifier::add_entry(
            yaml,
            0,
            &["src".to_string()],
            true,
            "rust",
            &[],
        ).unwrap();

        assert!(result.contains("tree:"));
        assert!(result.contains("- name: src"));
    }
}
