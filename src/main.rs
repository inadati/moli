// start auto exported by moli.
mod project_management;
mod code_generation;
mod shared;
mod cli;
// end auto exported by moli.

use clap::Command;
use crate::cli::command;

fn main() -> anyhow::Result<()> {

    let version = env!("CARGO_PKG_VERSION");

    let matches = Command::new("moli")
        .about(&format!("Moli v{} - A declarative development framework for multi-language project generation", version))
        .long_about(
            "Moli is a declarative development framework that generates project structures \
            from YAML specifications. It supports Rust, Go, Python, TypeScript, and JavaScript \
            with intelligent project structure generation, workspace management, and \
            language-specific configurations."
        )
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            command::up::spec()
        )
        .subcommand(
            command::new::spec()
        )
        .subcommand(
            command::rm::spec()
        )
        .subcommand(
            command::load::spec()
        )
        .subcommand(
            command::claude_skill::spec()
        )
        .subcommand(
            command::completion::spec()
        )
        .version(version)
        .get_matches();

    match matches.subcommand() {
        Some(("up", _)) => {
            command::up::action()
        }
        Some(("new", sub_matches)) => {
            command::new::action(sub_matches)
        }
        Some(("rm", _)) => {
            command::rm::action()
        }
        Some(("load", _)) => {
            command::load::action()
        }
        Some(("claude-skill", _)) => {
            command::claude_skill::action()
        }
        Some(("completion", sub_matches)) => {
            command::completion::action(sub_matches)
        }
        _ => unreachable!()
    }
}
