use clap::{Command, Arg, ValueEnum};
use clap_complete::{generate, Generator, Shell};
use anyhow::Result;
use std::io;

#[derive(Debug, Clone, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

impl From<CompletionShell> for Shell {
    fn from(shell: CompletionShell) -> Self {
        match shell {
            CompletionShell::Bash => Shell::Bash,
            CompletionShell::Zsh => Shell::Zsh,
            CompletionShell::Fish => Shell::Fish,
            CompletionShell::PowerShell => Shell::PowerShell,
            CompletionShell::Elvish => Shell::Elvish,
        }
    }
}

pub fn spec() -> Command {
    Command::new("completion")
        .about("Generate shell completion scripts")
        .long_about(
            "Generate shell completion scripts for moli.\n\n\
            Installation examples:\n  \
            Bash:       moli completion bash > /usr/local/etc/bash_completion.d/moli\n  \
            Zsh:        moli completion zsh > ~/.zsh/completion/_moli\n  \
            Fish:       moli completion fish > ~/.config/fish/completions/moli.fish\n  \
            PowerShell: moli completion powershell > moli.ps1"
        )
        .arg(
            Arg::new("shell")
                .help("Shell type to generate completion for")
                .value_parser(clap::value_parser!(CompletionShell))
                .required(true)
        )
}

pub fn action(matches: &clap::ArgMatches) -> Result<()> {
    let shell = matches.get_one::<CompletionShell>("shell")
        .expect("Shell argument is required");

    let mut cmd = build_cli();
    let bin_name = cmd.get_name().to_string();

    let shell_type: Shell = shell.clone().into();
    print_completions(shell_type, &mut cmd, bin_name);

    Ok(())
}

fn print_completions<G: Generator>(gen: G, cmd: &mut Command, bin_name: String) {
    generate(gen, cmd, bin_name, &mut io::stdout());
}

// CLIの構造を再現（main.rsと同じ構造）
fn build_cli() -> Command {
    let version = env!("CARGO_PKG_VERSION");

    Command::new("moli")
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
            Command::new("up")
                .about("Generate project structure from moli.yml configuration")
        )
        .subcommand(
            Command::new("new")
                .about("Initialize a new project with moli.yml configuration")
                .arg(
                    Arg::new("lang")
                        .long("lang")
                        .help("Programming language")
                        .value_parser(["rust", "go", "python", "typescript", "javascript", "markdown"])
                )
        )
.subcommand(
            Command::new("scan")
                .about("Scan and import unmanaged files or directories into moli.yml")
        )
        .subcommand(
            Command::new("rm")
                .about("Remove files and directories deleted from moli.yml")
        )
        .subcommand(spec())
        .version(version)
}
