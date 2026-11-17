mod api;
mod cli;
mod country;
mod scrape;
mod search;

use api::SerperClient;
use std::io::{self, Write};

fn main() {
    let matches = match cli::command().try_get_matches() {
        Ok(matches) => matches,
        Err(error) => {
            let code = error.exit_code();
            let _ = error.print();
            std::process::exit(code);
        }
    };
    let (name, args) = matches.subcommand().expect("subcommand is required");
    let timeout = match cli::timeout(args) {
        Ok(timeout) => timeout,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let client = match SerperClient::new(std::env::var("SERPER_KEY").unwrap_or_default(), timeout) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("{}", error.message);
            std::process::exit(1);
        }
    };
    let code = cli::execute(
        name,
        args,
        &client,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    );
    let _ = io::stdout().flush();
    std::process::exit(code);
}

#[cfg(test)]
mod integration_tests;
