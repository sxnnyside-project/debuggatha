//! `debuggatha completion`: shell completion scripts.

use std::io;

use clap::{CommandFactory, ValueEnum};
use clap_complete::{Shell, generate};

use crate::Cli;

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Target {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

/// Print a completion script for the shell, generated from the command definitions themselves.
pub(crate) fn run(target: Target) {
    let shell = match target {
        Target::Bash => Shell::Bash,
        Target::Zsh => Shell::Zsh,
        Target::Fish => Shell::Fish,
        Target::Powershell => Shell::PowerShell,
    };
    generate(shell, &mut Cli::command(), "debuggatha", &mut io::stdout());
}
