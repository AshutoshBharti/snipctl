/// Interactive TUI for fuzzy-searching snippets — crossterm based.

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};

use crate::fuzzy::fuzzy_filter;
use crate::store::Snippet;

// ANSI helpers for inline styling
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

/// Launch an interactive fuzzy-search TUI. Returns index of selected snippet or None.
pub fn interactive_search(snippets: &[Snippet], cli_filter: Option<&str>) -> Option<usize> {
    if !atty::is(atty::Stream::Stdin) {
        eprintln!("Error: interactive mode requires a terminal.");
        return None;
    }

    let filtered_snippets: Vec<&Snippet> = match cli_filter {
        Some(cli) => snippets.iter().filter(|s| s.cli == cli).collect(),
        None => snippets.iter().collect(),
    };

    if filtered_snippets.is_empty() {
        println!("{YELLOW}No snippets saved yet.{RESET}");
        println!("Run a command or use: {CYAN}snipctl save \"<command>\"{RESET}");
        return None;
    }

    let mut query = String::new();
    let mut cursor_pos: usize = 0;
    let (_cols, rows) = terminal::size().unwrap_or((80, 24));
    let max_visible = (rows as usize).saturating_sub(4).min(20);

    terminal::enable_raw_mode().ok()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide).ok();

    let result = run_search_loop(
        &filtered_snippets,
        &mut query,
        &mut cursor_pos,
        max_visible,
    );

    execute!(stdout, cursor::Show, LeaveAlternateScreen).ok();
    terminal::disable_raw_mode().ok();

    // Map back to original index
    result.and_then(|filtered_idx| {
        let selected = filtered_snippets.get(filtered_idx)?;
        snippets.iter().position(|s| s.id == selected.id)
    })
}

fn run_search_loop(
    snippets: &[&Snippet],
    query: &mut String,
    cursor_pos: &mut usize,
    max_visible: usize,
) -> Option<usize> {
    let mut stdout = io::stdout();

    loop {
        let indices = fuzzy_filter(query, snippets, |s| {
            let mut fields = vec![s.template.clone(), s.description.clone()];
            fields.push(s.tags.join(" "));
            fields.push(s.cli.clone());
            fields
        });

        if *cursor_pos >= indices.len() {
            *cursor_pos = indices.len().saturating_sub(1);
        }

        // Build frame as lines, then join with \r\n
        let mut lines: Vec<String> = Vec::new();

        // Header
        lines.push(format!("{CYAN}❯ snipctl search:{RESET} {query}█"));

        // Status
        lines.push(format!(
            "{DIM}  {}/{} snippets  ↑↓ navigate  Enter select  Esc quit{RESET}",
            indices.len(),
            snippets.len()
        ));

        // Blank line
        lines.push(String::new());

        // Snippet list
        let visible_start = cursor_pos.saturating_sub(max_visible.saturating_sub(1));
        let visible_end = indices.len().min(visible_start + max_visible);

        for (display_idx, &item_idx) in indices[visible_start..visible_end].iter().enumerate() {
            let actual_display = visible_start + display_idx;
            let s = snippets[item_idx];
            let mut line = String::new();

            if actual_display == *cursor_pos {
                line.push_str(&format!("{CYAN}▸{RESET} "));
            } else {
                line.push_str("  ");
            }

            line.push_str(&format!("{DIM}[{}/{}]{RESET} ", s.cli, s.id));
            line.push_str(&highlight_placeholders(&s.template));

            if !s.description.is_empty() {
                line.push_str(&format!("  {DIM}# {}{RESET}", s.description));
            }

            lines.push(line);
        }

        // Home cursor + clear screen + write all lines joined by \r\n
        let frame = format!("\x1b[H\x1b[2J{}", lines.join("\r\n"));
        stdout.write_all(frame.as_bytes()).ok();
        stdout.flush().ok();

        // Input
        if let Ok(Event::Key(key)) = event::read() {
            match key {
                KeyEvent {
                    code: KeyCode::Up, ..
                } => {
                    *cursor_pos = cursor_pos.saturating_sub(1);
                }
                KeyEvent {
                    code: KeyCode::Down,
                    ..
                } => {
                    if *cursor_pos + 1 < indices.len() {
                        *cursor_pos += 1;
                    }
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    if !indices.is_empty() {
                        return Some(indices[*cursor_pos]);
                    }
                    return None;
                }
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => {
                    return None;
                }
                KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    return None;
                }
                KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                } => {
                    query.pop();
                    *cursor_pos = 0;
                }
                KeyEvent {
                    code: KeyCode::Char(ch),
                    ..
                } => {
                    query.push(ch);
                    *cursor_pos = 0;
                }
                _ => {}
            }
        }
    }
}

fn highlight_placeholders(template: &str) -> String {
    let mut result = String::new();
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                let mut placeholder = String::from("{{");
                while let Some(&next) = chars.peek() {
                    placeholder.push(chars.next().unwrap());
                    if next == '}' && chars.peek() == Some(&'}') {
                        placeholder.push(chars.next().unwrap());
                        break;
                    }
                }
                result.push_str(&format!("{GREEN}{placeholder}{RESET}"));
            } else {
                result.push(ch);
            }
        } else {
            result.push(ch);
        }
    }

    result
}
