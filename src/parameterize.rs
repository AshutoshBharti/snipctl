/// Parameterize CLI commands — turn flag values into {{placeholder}} templates.
///
/// Ported from azsnip's parameterize.py — works for any CLI.

use regex::Regex;

/// Convert a CLI command into a parameterized template.
///
/// Rules:
///   --flag value        → --flag {{flag}}
///   --flag=value        → --flag={{flag}}
///   -f value            → -f {{f}}
///   Positional subcommands (before first flag) are kept as-is.
///   Values that look like flags are not replaced.
pub fn parameterize(command: &str) -> String {
    let tokens = shell_split(command);
    if tokens.is_empty() {
        return command.to_string();
    }

    let mut result: Vec<String> = Vec::new();
    let mut i = 0;

    // preserve leading command + subcommands (az group create, aws ec2 ...)
    while i < tokens.len() && !tokens[i].starts_with('-') {
        result.push(tokens[i].clone());
        i += 1;
    }

    while i < tokens.len() {
        let token = &tokens[i];

        if token.starts_with('-') && token.contains('=') {
            // --flag=value
            let (flag, _val) = token.split_once('=').unwrap();
            let name = flag_name(flag);
            result.push(format!("{flag}={{{{{name}}}}}"));
            i += 1;
        } else if token.starts_with('-') {
            let flag = token;
            let name = flag_name(flag);
            // check if next token is a value (not another flag)
            if i + 1 < tokens.len() && !tokens[i + 1].starts_with('-') {
                result.push(flag.clone());
                result.push(format!("{{{{{name}}}}}"));
                i += 2;
            } else {
                // boolean flag (no value)
                result.push(flag.clone());
                i += 1;
            }
        } else {
            // positional arg in flag area — keep as-is
            result.push(token.clone());
            i += 1;
        }
    }

    result.join(" ")
}

/// Return ordered list of placeholder names from a template.
pub fn extract_placeholders(template: &str) -> Vec<String> {
    let re = Regex::new(r"\{\{(\w+)\}\}").unwrap();
    re.captures_iter(template)
        .map(|c| c[1].to_string())
        .collect()
}

/// Replace {{placeholder}} tokens with provided values.
pub fn fill_template(template: &str, values: &std::collections::HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, val) in values {
        let safe_val = if val.contains(' ') {
            format!("\"{}\"", val)
        } else {
            val.clone()
        };
        result = result.replace(&format!("{{{{{key}}}}}"), &safe_val);
    }
    result
}

/// Derive a placeholder name from a flag like --resource-group or -g.
fn flag_name(flag: &str) -> String {
    flag.trim_start_matches('-').replace('-', "_")
}

/// Simple shell-like token splitting (handles single and double quotes).
fn shell_split(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            ' ' | '\t' if !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            '\\' if !in_single => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_parameterize_az() {
        assert_eq!(
            parameterize("az group create --name myRG --location eastus"),
            "az group create --name {{name}} --location {{location}}"
        );
    }

    #[test]
    fn test_parameterize_aws() {
        assert_eq!(
            parameterize("aws ec2 describe-instances --instance-id i-123"),
            "aws ec2 describe-instances --instance-id {{instance_id}}"
        );
    }

    #[test]
    fn test_parameterize_gcloud() {
        assert_eq!(
            parameterize("gcloud compute instances list --zone us-central1-a"),
            "gcloud compute instances list --zone {{zone}}"
        );
    }

    #[test]
    fn test_parameterize_equals() {
        assert_eq!(
            parameterize("az storage blob upload --account-name=myaccount"),
            "az storage blob upload --account-name={{account_name}}"
        );
    }

    #[test]
    fn test_parameterize_boolean_flag() {
        assert_eq!(
            parameterize("az group list --verbose"),
            "az group list --verbose"
        );
    }

    #[test]
    fn test_extract_placeholders() {
        let placeholders =
            extract_placeholders("az group create --name {{name}} --location {{location}}");
        assert_eq!(placeholders, vec!["name", "location"]);
    }

    #[test]
    fn test_fill_template() {
        let mut values = HashMap::new();
        values.insert("name".into(), "myRG".into());
        values.insert("location".into(), "eastus".into());
        assert_eq!(
            fill_template(
                "az group create --name {{name}} --location {{location}}",
                &values
            ),
            "az group create --name myRG --location eastus"
        );
    }

    #[test]
    fn test_fill_template_with_spaces() {
        let mut values = HashMap::new();
        values.insert("name".into(), "my resource group".into());
        let filled = fill_template("az group create --name {{name}}", &values);
        assert_eq!(filled, "az group create --name \"my resource group\"");
    }

    #[test]
    fn test_shell_split_quotes() {
        let tokens = shell_split(r#"az group create --name "my rg" --location eastus"#);
        assert_eq!(
            tokens,
            vec!["az", "group", "create", "--name", "my rg", "--location", "eastus"]
        );
    }

    #[test]
    fn test_parameterize_gh() {
        assert_eq!(
            parameterize("gh pr create --title my-pr --base main"),
            "gh pr create --title {{title}} --base {{base}}"
        );
    }
}
