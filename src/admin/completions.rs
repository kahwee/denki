use crate::cli::Cli;
use clap::CommandFactory;
use clap_complete::generate;

pub fn handle_completions(shell: clap_complete::Shell) {
    generate(shell, &mut Cli::command(), "denki", &mut std::io::stdout());
}
