const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

pub fn show_diff(old: &str, new: &str) {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let mut old_idx = 0;
    let mut new_idx = 0;

    while old_idx < old_lines.len() || new_idx < new_lines.len() {
        if old_idx < old_lines.len() && new_idx < new_lines.len() {
            if old_lines[old_idx] == new_lines[new_idx] {
                println!("  {}", old_lines[old_idx]);
                old_idx += 1;
                new_idx += 1;
            } else {
                let mut found = false;
                for ahead in 0..old_lines.len() - old_idx {
                    if new_idx < new_lines.len() && old_lines[old_idx + ahead] == new_lines[new_idx] {
                        for k in 0..ahead {
                            println!("{}- {}{}", RED, old_lines[old_idx + k], RESET);
                        }
                        old_idx += ahead;
                        found = true;
                        break;
                    }
                }
                if !found {
                    println!("{}- {}{}", RED, old_lines[old_idx], RESET);
                    old_idx += 1;
                }
            }
        } else if old_idx < old_lines.len() {
            println!("{}- {}{}", RED, old_lines[old_idx], RESET);
            old_idx += 1;
        } else {
            println!("{}+ {}{}", GREEN, new_lines[new_idx], RESET);
            new_idx += 1;
        }
    }
}
