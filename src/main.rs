mod api;
mod cli;
mod country;
mod scrape;
mod search;

use api::SerperClient;
use std::io::{self, Write};

fn main() {
    clap_complete::CompleteEnv::with_factory(cli::command)
        .var("SERPER_COMPLETE")
        .complete();

    let matches = match cli::command().try_get_matches() {
        Ok(matches) => matches,
        Err(error) => {
            let code = error.exit_code();
            let _ = error.print();
            std::process::exit(code);
        }
    };
    let (name, args) = matches.subcommand().expect("subcommand is required");
    if name == "completion" {
        let shell = *args
            .get_one::<clap_complete::Shell>("shell")
            .expect("shell is required");
        let mut stdout = io::stdout().lock();
        match shell {
            clap_complete::Shell::PowerShell => stdout
                .write_all(include_bytes!("completion.ps1"))
                .expect("failed to write completion script"),
            clap_complete::Shell::Fish | clap_complete::Shell::Elvish => {
                let shells = clap_complete::env::Shells::builtins();
                shells
                    .completer(&shell.to_string())
                    .expect("supported shell")
                    .write_registration(
                        "SERPER_COMPLETE",
                        "serper",
                        "serper",
                        "serper",
                        &mut stdout,
                    )
                    .expect("failed to write completion script");
            }
            _ => clap_complete::generate(shell, &mut cli::command(), "serper", &mut stdout),
        }
        return;
    }
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
