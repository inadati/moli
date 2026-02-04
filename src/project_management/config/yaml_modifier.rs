use anyhow::{Result, bail};
use crate::project_management::config::path_collector::ManagedFile;

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
}
