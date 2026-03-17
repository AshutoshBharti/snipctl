mod cli;
mod config;
mod fuzzy;
mod hooks;
mod parameterize;
mod runner;
mod store;
mod tui;

fn main() {
    cli::run();
}
