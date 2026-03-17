/// Dynamic shell hook generation for any CLI.

/// Generate a bash/zsh hook that wraps a CLI command for auto-capture.
pub fn hook_bash(cli_name: &str) -> String {
    format!(
        r#"# snipctl: auto-capture {cli_name} commands
{cli_name}() {{
    local cmd="{cli_name} $*"
    command {cli_name} "$@"
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        snipctl capture --cli {cli_name} "$cmd" 2>/dev/null &
    fi
    return $exit_code
}}"#
    )
}

/// Generate a fish hook that wraps a CLI command for auto-capture.
pub fn hook_fish(cli_name: &str) -> String {
    format!(
        r#"# snipctl: auto-capture {cli_name} commands
function {cli_name} --wraps='{cli_name}'
    set -l cmd "{cli_name} $argv"
    command {cli_name} $argv
    set -l exit_code $status
    if test $exit_code -eq 0
        snipctl capture --cli {cli_name} "$cmd" 2>/dev/null &
    end
    return $exit_code
end"#
    )
}

/// Generate a PowerShell hook that wraps a CLI command for auto-capture.
pub fn hook_powershell(cli_name: &str) -> String {
    let fn_name = capitalize(cli_name);
    // For az on Windows, the actual executable is az.cmd
    let exe = if cli_name == "az" {
        "az.cmd".to_string()
    } else {
        cli_name.to_string()
    };

    format!(
        r#"# snipctl: auto-capture {cli_name} commands
function Invoke-{fn_name}Wrapped {{
    $cmd = "{cli_name} $($args -join ' ')"
    & {exe} @args
    if ($LASTEXITCODE -eq 0) {{
        Start-Job -ScriptBlock {{ snipctl capture --cli {cli_name} $using:cmd }} | Out-Null
    }}
}}
Set-Alias -Name {cli_name} -Value Invoke-{fn_name}Wrapped -Option AllScope"#
    )
}

/// Generate hooks for all configured CLIs for a given shell type.
pub fn generate_hooks(cli_names: &[&str], shell: &str) -> String {
    let hook_fn = match shell {
        "bash" | "zsh" => hook_bash as fn(&str) -> String,
        "fish" => hook_fish,
        "powershell" => hook_powershell,
        _ => hook_bash,
    };

    cli_names
        .iter()
        .map(|name| hook_fn(name))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Get the config file path hint for a shell.
pub fn config_file_hint(shell: &str) -> &str {
    match shell {
        "bash" => "~/.bashrc",
        "zsh" => "~/.zshrc",
        "fish" => "~/.config/fish/conf.d/snipctl.fish",
        "powershell" => "$PROFILE",
        _ => "~/.bashrc",
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_hook_az() {
        let hook = hook_bash("az");
        assert!(hook.contains("az()"));
        assert!(hook.contains("command az"));
        assert!(hook.contains("snipctl capture --cli az"));
    }

    #[test]
    fn test_bash_hook_aws() {
        let hook = hook_bash("aws");
        assert!(hook.contains("aws()"));
        assert!(hook.contains("command aws"));
        assert!(hook.contains("snipctl capture --cli aws"));
    }

    #[test]
    fn test_fish_hook_gcloud() {
        let hook = hook_fish("gcloud");
        assert!(hook.contains("function gcloud --wraps='gcloud'"));
        assert!(hook.contains("snipctl capture --cli gcloud"));
    }

    #[test]
    fn test_powershell_hook() {
        let hook = hook_powershell("az");
        assert!(hook.contains("Invoke-AzWrapped"));
        assert!(hook.contains("az.cmd"));
        assert!(hook.contains("snipctl capture --cli az"));
    }

    #[test]
    fn test_powershell_hook_aws() {
        let hook = hook_powershell("aws");
        assert!(hook.contains("Invoke-AwsWrapped"));
        assert!(!hook.contains("aws.cmd")); // only az uses .cmd
        assert!(hook.contains("& aws"));
    }

    #[test]
    fn test_generate_multiple_hooks() {
        let hooks = generate_hooks(&["az", "aws", "gcloud"], "bash");
        assert!(hooks.contains("az()"));
        assert!(hooks.contains("aws()"));
        assert!(hooks.contains("gcloud()"));
    }
}
