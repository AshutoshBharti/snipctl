/// CLI entry-point — all command routing via clap.

use clap::{Parser, Subcommand};
use std::io::Write;
use std::path::PathBuf;
use std::process;

use crate::config::Config;
use crate::fuzzy::fuzzy_filter;
use crate::hooks;
use crate::parameterize::parameterize;
use crate::runner;
use crate::store::SnippetStore;
use crate::tui;

// ANSI helpers
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

#[derive(Parser)]
#[command(
    name = "snipctl",
    about = "Universal CLI Snippet Manager — capture, search, and reuse commands from any CLI.",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive fuzzy search (default when no subcommand given)
    Search {
        /// Optional initial query
        #[arg(default_value = "")]
        query: String,
        /// Filter by CLI (e.g. az, aws, gcloud)
        #[arg(long)]
        cli: Option<String>,
    },
    /// List all saved snippets
    #[command(alias = "ls")]
    List {
        /// Filter with fuzzy query
        #[arg(short, long, default_value = "")]
        query: String,
        /// Filter by CLI
        #[arg(long)]
        cli: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Manually save a command
    Save {
        /// The command to save (quote it)
        cmd: String,
        /// Description
        #[arg(short, long, default_value = "")]
        description: String,
        /// CLI name (auto-detected if omitted)
        #[arg(long)]
        cli: Option<String>,
    },
    /// Capture a command (used by shell hooks)
    Capture {
        /// The command to capture
        cmd: String,
        /// CLI name
        #[arg(long, default_value = "unknown")]
        cli: String,
    },
    /// Run a saved snippet
    Run {
        /// Snippet ID
        id: String,
        /// Print command without executing
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete a snippet
    #[command(alias = "rm")]
    Delete {
        /// Snippet ID
        id: String,
    },
    /// Edit a snippet's template or description
    Edit {
        /// Snippet ID
        id: String,
        /// New template
        #[arg(short, long)]
        template: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Export snippets as JSON
    Export {
        /// Filter by CLI
        #[arg(long)]
        cli: Option<String>,
    },
    /// Import snippets from JSON file
    Import {
        /// Path to JSON file
        file: PathBuf,
        /// Default CLI for imported snippets without a cli field
        #[arg(long, default_value = "az")]
        default_cli: String,
    },
    /// Print shell hook for auto-capturing commands
    Hook {
        /// Target shell (auto-detected if omitted)
        #[arg(value_parser = ["bash", "zsh", "fish", "powershell"])]
        shell: Option<String>,
        /// Generate hook for specific CLI only
        #[arg(long)]
        cli: Option<String>,
    },
    /// Manage tracked CLIs
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Add a CLI to track
    Add {
        /// CLI prefix (e.g. gh, kubectl, docker)
        prefix: String,
    },
    /// Remove a CLI from tracking
    Remove {
        /// CLI prefix
        prefix: String,
    },
    /// List tracked CLIs
    List,
    /// Show config file path
    Path,
}

pub fn run() {
    let args = Cli::parse();
    let config = Config::load();
    let store = SnippetStore::new(config.storage_path());

    match args.command {
        None => cmd_search(&store, "", None),
        Some(Commands::Search { query, cli }) => cmd_search(&store, &query, cli.as_deref()),
        Some(Commands::List { query, cli, json }) => cmd_list(&store, &query, cli.as_deref(), json),
        Some(Commands::Save {
            cmd,
            description,
            cli,
        }) => cmd_save(&store, &config, &cmd, &description, cli.as_deref()),
        Some(Commands::Capture { cmd, cli }) => cmd_capture(&store, &cmd, &cli),
        Some(Commands::Run { id, dry_run }) => cmd_run(&store, &id, dry_run),
        Some(Commands::Delete { id }) => cmd_delete(&store, &id),
        Some(Commands::Edit {
            id,
            template,
            description,
        }) => cmd_edit(&store, &id, template.as_deref(), description.as_deref()),
        Some(Commands::Export { cli }) => cmd_export(&store, cli.as_deref()),
        Some(Commands::Import { file, default_cli }) => cmd_import(&store, &file, &default_cli),
        Some(Commands::Hook { shell, cli }) => cmd_hook(&config, shell.as_deref(), cli.as_deref()),
        Some(Commands::Config { action }) => cmd_config(config, action),
    }
}

fn cmd_search(store: &SnippetStore, _query: &str, cli_filter: Option<&str>) {
    let snippets = store.all();
    if let Some(idx) = tui::interactive_search(&snippets, cli_filter) {
        let snippet = &snippets[idx];
        println!("\n{CYAN}Selected:{RESET} {}", snippet.template);

        if let Some((_, _)) = runner::run_snippet(&snippet.template, false) {
            store.touch(&snippet.id);
        }
    }
}

fn cmd_list(store: &SnippetStore, query: &str, cli_filter: Option<&str>, as_json: bool) {
    let snippets = match cli_filter {
        Some(cli) => store.all_by_cli(cli),
        None => store.all(),
    };

    let display_indices = if query.is_empty() {
        (0..snippets.len()).collect::<Vec<_>>()
    } else {
        fuzzy_filter(query, &snippets, |s| {
            vec![s.template.clone(), s.description.clone(), s.tags.join(" ")]
        })
    };

    if as_json {
        let filtered: Vec<&_> = display_indices.iter().map(|&i| &snippets[i]).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&filtered).unwrap_or_default()
        );
        return;
    }

    if display_indices.is_empty() {
        println!("{YELLOW}No snippets found.{RESET}");
        return;
    }

    for &idx in &display_indices {
        let s = &snippets[idx];
        let desc = if s.description.is_empty() {
            String::new()
        } else {
            format!("  {DIM}# {}{RESET}", s.description)
        };
        let uses = format!("{DIM}(used {}x){RESET}", s.usage_count);
        println!(
            "  {DIM}[{}/{}]{RESET} {}{desc}  {uses}",
            s.cli, s.id, s.template
        );
    }
}

fn cmd_save(
    store: &SnippetStore,
    config: &Config,
    cmd: &str,
    description: &str,
    cli_override: Option<&str>,
) {
    let template = parameterize(cmd);
    let cli = cli_override
        .map(|s| s.to_string())
        .or_else(|| config.detect_cli(cmd))
        .unwrap_or_else(|| "unknown".to_string());

    let snippet = store.add(&template, cmd, description, None, &cli);
    println!("{GREEN}✓ Saved:{RESET} {}", snippet.template);
    println!("  {DIM}CLI: {} | ID: {}{RESET}", snippet.cli, snippet.id);
}

fn cmd_capture(store: &SnippetStore, cmd: &str, cli: &str) {
    let template = parameterize(cmd);
    store.add(&template, cmd, "", None, cli);
}

fn cmd_run(store: &SnippetStore, id: &str, dry_run: bool) {
    let snippet = match store.get(id) {
        Some(s) => s,
        None => {
            eprintln!("{RED}✗ Snippet '{id}' not found.{RESET}");
            process::exit(1);
        }
    };

    println!("{CYAN}Template:{RESET} {}", snippet.template);

    if let Some((_, _)) = runner::run_snippet(&snippet.template, dry_run) {
        if !dry_run {
            store.touch(&snippet.id);
        }
    }
}

fn cmd_delete(store: &SnippetStore, id: &str) {
    let snippet = match store.get(id) {
        Some(s) => s,
        None => {
            eprintln!("{RED}✗ Snippet '{id}' not found.{RESET}");
            process::exit(1);
        }
    };

    println!(
        "  {DIM}[{}/{}]{RESET} {}",
        snippet.cli, snippet.id, snippet.template
    );

    print!("{YELLOW}Delete? [y/N]:{RESET} ");
    std::io::stdout().flush().ok();

    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return;
    }
    let answer = answer.trim().to_lowercase();

    if answer == "y" || answer == "yes" {
        store.delete(id);
        println!("{GREEN}✓ Deleted.{RESET}");
    }
}

fn cmd_edit(store: &SnippetStore, id: &str, template: Option<&str>, description: Option<&str>) {
    let snippet = match store.get(id) {
        Some(s) => s,
        None => {
            eprintln!("{RED}✗ Snippet '{id}' not found.{RESET}");
            process::exit(1);
        }
    };

    if template.is_none() && description.is_none() {
        // Interactive edit
        println!("Current template: {}", snippet.template);
        print!("New template (Enter to keep): ");
        std::io::stdout().flush().ok();
        let mut new_tmpl = String::new();
        std::io::stdin().read_line(&mut new_tmpl).ok();
        let new_tmpl = new_tmpl.trim();

        print!("New description (Enter to keep): ");
        std::io::stdout().flush().ok();
        let mut new_desc = String::new();
        std::io::stdin().read_line(&mut new_desc).ok();
        let new_desc = new_desc.trim();

        let t = if new_tmpl.is_empty() {
            None
        } else {
            Some(new_tmpl)
        };
        let d = if new_desc.is_empty() {
            None
        } else {
            Some(new_desc)
        };

        if t.is_some() || d.is_some() {
            if let Some(updated) = store.update(id, t, d, None) {
                println!("{GREEN}✓ Updated:{RESET} {}", updated.template);
            }
        } else {
            println!("No changes.");
        }
    } else {
        if let Some(updated) = store.update(id, template, description, None) {
            println!("{GREEN}✓ Updated:{RESET} {}", updated.template);
        }
    }
}

fn cmd_export(store: &SnippetStore, cli_filter: Option<&str>) {
    println!("{}", store.export_all(cli_filter));
}

fn cmd_import(store: &SnippetStore, file: &PathBuf, default_cli: &str) {
    let data = match std::fs::read_to_string(file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "{RED}✗ Failed to read {}: {e}{RESET}",
                file.display()
            );
            process::exit(1);
        }
    };

    match store.import_from(&data, default_cli) {
        Ok(count) => println!("{GREEN}✓ Imported {count} snippet(s).{RESET}"),
        Err(e) => {
            eprintln!("{RED}✗ Import failed: {e}{RESET}");
            process::exit(1);
        }
    }
}

fn cmd_hook(config: &Config, shell: Option<&str>, cli_filter: Option<&str>) {
    let shell = shell
        .map(String::from)
        .unwrap_or_else(|| detect_shell());

    let cli_names: Vec<&str> = match cli_filter {
        Some(cli) => vec![cli],
        None => config.cli.iter().map(|c| c.prefix.as_str()).collect(),
    };

    println!("{CYAN}Shell hooks for {shell}{RESET}");
    println!(
        "{DIM}Add this to {}:{RESET}\n",
        hooks::config_file_hint(&shell)
    );
    println!("{}", hooks::generate_hooks(&cli_names, &shell));
    println!(
        "\n{DIM}Then reload your shell or run: source {}{RESET}",
        hooks::config_file_hint(&shell)
    );
}

fn cmd_config(mut config: Config, action: ConfigAction) {
    match action {
        ConfigAction::Add { prefix } => {
            if config.has_cli(&prefix) {
                println!("{YELLOW}CLI '{prefix}' is already tracked.{RESET}");
                return;
            }
            config.add_cli(&prefix);
            if let Err(e) = config.save() {
                eprintln!("{RED}✗ Failed to save config: {e}{RESET}");
                process::exit(1);
            }
            println!("{GREEN}✓ Added '{prefix}' to tracked CLIs.{RESET}");
            println!("{DIM}Run `snipctl hook` to get updated shell hooks.{RESET}");
        }
        ConfigAction::Remove { prefix } => {
            if !config.remove_cli(&prefix) {
                println!("{YELLOW}CLI '{prefix}' is not tracked.{RESET}");
                return;
            }
            if let Err(e) = config.save() {
                eprintln!("{RED}✗ Failed to save config: {e}{RESET}");
                process::exit(1);
            }
            println!("{GREEN}✓ Removed '{prefix}' from tracked CLIs.{RESET}");
        }
        ConfigAction::List => {
            println!("{CYAN}Tracked CLIs:{RESET}");
            for cli in &config.cli {
                println!("  • {}{RESET}", cli.prefix);
            }
            println!(
                "\n{DIM}Add more with: snipctl config add <prefix>{RESET}"
            );
        }
        ConfigAction::Path => {
            println!("{}", Config::config_path().display());
        }
    }
}

fn detect_shell() -> String {
    if let Ok(shell) = std::env::var("SHELL") {
        if shell.contains("zsh") {
            return "zsh".into();
        }
        if shell.contains("fish") {
            return "fish".into();
        }
        if shell.contains("bash") {
            return "bash".into();
        }
    }
    if cfg!(target_os = "windows") {
        return "powershell".into();
    }
    "bash".into()
}
