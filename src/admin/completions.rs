use clap::CommandFactory;
use clap_complete::{Shell, generate};
pub fn handle_completions(shell: Shell) {
    let mut buffer = Vec::new();
    generate(shell, &mut crate::cli::Cli::command(), "denki", &mut buffer);
    let script = String::from_utf8_lossy(&buffer);
    crate::output::record(serde_json::json!({"script":script}));
    if !crate::output::is_json() {
        print!("{script}");
    }
}
