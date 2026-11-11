#[tokio::main]
async fn main() -> std::process::ExitCode {
    match denki::app::run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "Error: {}",
                denki::output::terminal_text(&format!("{error:#}"))
            );
            std::process::ExitCode::FAILURE
        }
    }
}
