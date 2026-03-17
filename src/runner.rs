/// Template filling and command execution.

use std::collections::HashMap;
use std::io::{self, Write};
use std::process::Command;

use crate::parameterize::{extract_placeholders, fill_template};

// ANSI helpers
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

/// Interactively prompt user for placeholder values.
pub fn prompt_placeholders(template: &str) -> Option<HashMap<String, String>> {
    let placeholders = extract_placeholders(template);
    if placeholders.is_empty() {
        return Some(HashMap::new());
    }

    println!("\n{CYAN}Fill in placeholders:{RESET}");
    println!("{}", highlight_placeholders(template));

    let mut values = HashMap::new();
    let mut seen = std::collections::HashSet::new();

    for ph in &placeholders {
        if seen.contains(ph) {
            continue;
        }
        seen.insert(ph.clone());

        print!("  {GREEN}{{{{{ph}}}}}{RESET} = ");
        io::stdout().flush().ok();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) | Err(_) => return None,
            _ => {}
        }
        values.insert(ph.clone(), input.trim().to_string());
    }

    Some(values)
}

/// Execute a filled command string.
pub fn execute_command(command: &str) -> i32 {
    let shell = if cfg!(target_os = "windows") {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };

    match Command::new(shell.0).arg(shell.1).arg(command).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            eprintln!("{YELLOW}Error executing command: {e}{RESET}");
            1
        }
    }
}

/// Run the full flow: prompt for placeholders, confirm, execute.
pub fn run_snippet(template: &str, dry_run: bool) -> Option<(String, i32)> {
    let values = prompt_placeholders(template)?;
    let filled = fill_template(template, &values);

    println!("\n{GREEN}Command:{RESET} {filled}");

    if dry_run {
        return Some((filled, 0));
    }

    print!("\n{YELLOW}Run it? [Y/n]:{RESET} ");
    io::stdout().flush().ok();

    let mut answer = String::new();
    match io::stdin().read_line(&mut answer) {
        Ok(0) | Err(_) => return None,
        _ => {}
    }
    let answer = answer.trim().to_lowercase();

    if answer.is_empty() || answer == "y" || answer == "yes" {
        let code = execute_command(&filled);
        Some((filled, code))
    } else {
        None
    }
}

fn highlight_placeholders(template: &str) -> String {
    let re = regex::Regex::new(r"\{\{\w+\}\}").unwrap();
    re.replace_all(template, |caps: &regex::Captures| {
        format!("{GREEN}{}{RESET}", &caps[0])
    })
    .to_string()
}
