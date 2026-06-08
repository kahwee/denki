pub(super) fn parse_hint(hint: &str) -> Result<crate::cli::Cli, clap::Error> {
    use clap::Parser;
    let args: Vec<String> = hint
        .split_whitespace()
        .map(|t| t.trim_matches('"').to_string())
        .collect();
    crate::cli::Cli::try_parse_from(args)
}
