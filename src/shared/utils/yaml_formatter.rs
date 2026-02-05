/// Add blank lines between projects in YAML output
pub fn add_project_spacing(yaml: &str) -> String {
    // Add blank line before each project (except the first one)
    // Pattern: "\n- name:" -> "\n\n- name:"
    let lines: Vec<&str> = yaml.lines().collect();
    let mut result = String::new();
    let mut is_first_project = true;

    for line in lines {
        if line.starts_with("- name:") {
            if !is_first_project {
                result.push('\n'); // Add 3 blank lines between projects
                result.push('\n');
                result.push('\n');
            }
            is_first_project = false;
        }
        result.push_str(line);
        result.push('\n');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_project_spacing() {
        let input = r#"- name: project1
  lang: rust
- name: project2
  lang: go
"#;

        let expected = r#"- name: project1
  lang: rust



- name: project2
  lang: go
"#;

        assert_eq!(add_project_spacing(input), expected);
    }

    #[test]
    fn test_add_project_spacing_single_project() {
        let input = r#"- name: project1
  lang: rust
"#;

        let expected = r#"- name: project1
  lang: rust
"#;

        assert_eq!(add_project_spacing(input), expected);
    }
}
