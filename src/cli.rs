use crate::api::SerperClient;
use crate::scrape::{self, ScrapeOptions};
use crate::search::{self, SearchOptions};
use clap::{Arg, ArgMatches, Command};
use std::io::Write;

const DEFAULT_TIMEOUT: &str = "120";
const MAX_TIMEOUT: i64 = 9_223_372_036;

struct Spec {
    name: &'static str,
    endpoint: &'static str,
    description: &'static str,
    query: bool,
    country: bool,
    time: bool,
    extras: &'static [(&'static str, &'static str)],
}

const SPECS: &[Spec] = &[
    Spec {
        name: "general",
        endpoint: "search",
        description: "Search the web",
        query: true,
        country: true,
        time: true,
        extras: &[],
    },
    Spec {
        name: "image",
        endpoint: "images",
        description: "Search images",
        query: true,
        country: true,
        time: true,
        extras: &[],
    },
    Spec {
        name: "video",
        endpoint: "videos",
        description: "Search videos",
        query: true,
        country: true,
        time: true,
        extras: &[],
    },
    Spec {
        name: "place",
        endpoint: "places",
        description: "Search places",
        query: true,
        country: true,
        time: false,
        extras: &[("location", "Location hint")],
    },
    Spec {
        name: "maps",
        endpoint: "maps",
        description: "Search maps",
        query: true,
        country: true,
        time: false,
        extras: &[
            (
                "ll",
                "Latitude/longitude, required for multi-page query results",
            ),
            ("place-id", "Google place ID"),
            ("cid", "Google CID"),
        ],
    },
    Spec {
        name: "reviews",
        endpoint: "reviews",
        description: "Search reviews",
        query: false,
        country: true,
        time: false,
        extras: &[
            ("fid", "Google FID"),
            ("cid", "Google CID"),
            ("place-id", "Google place ID"),
            ("sort-by", "Review sort option"),
        ],
    },
    Spec {
        name: "news",
        endpoint: "news",
        description: "Search news",
        query: true,
        country: true,
        time: true,
        extras: &[],
    },
    Spec {
        name: "lens",
        endpoint: "lens",
        description: "Search by image",
        query: false,
        country: true,
        time: false,
        extras: &[("image-url", "Public image URL")],
    },
    Spec {
        name: "scholar",
        endpoint: "scholar",
        description: "Search scholarly publications",
        query: true,
        country: true,
        time: false,
        extras: &[],
    },
    Spec {
        name: "shopping",
        endpoint: "shopping",
        description: "Search shopping results",
        query: true,
        country: true,
        time: false,
        extras: &[],
    },
    Spec {
        name: "patents",
        endpoint: "patents",
        description: "Search patents",
        query: true,
        country: false,
        time: false,
        extras: &[],
    },
];

fn option(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .help(help)
        .num_args(1)
        .allow_hyphen_values(true)
}

fn timeout_arg() -> Arg {
    option("timeout", "Request timeout in seconds")
        .default_value(DEFAULT_TIMEOUT)
        .allow_hyphen_values(true)
}

fn common_search(
    command: Command,
    default_num: &'static str,
    query: bool,
    country: bool,
    time: bool,
) -> Command {
    let mut result = command
        .arg(
            option("search-num", "Number of results (1-100)")
                .default_value(default_num)
                .allow_hyphen_values(true),
        )
        .arg(timeout_arg());
    if query {
        result = result.arg(option("query", "Search keywords").required(true));
    }
    if country {
        result = result.arg(option("country", "Country name or ISO code (default: US)"));
        result = result.arg(option("language", "Language code, such as en"));
    }
    if time {
        result = result.arg(option("search-time", "hour, day, week, month, or year"));
    }
    result
}

pub fn command() -> Command {
    let mut root = Command::new("serper")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Search, scrape pages, and check Serper account credits")
        .after_help("The API key is read from SERPER_KEY.")
        .subcommand_required(true)
        .arg_required_else_help(true);
    root = root.subcommand(common_search(
        Command::new("aggregated").about("Search the web, news, and images"),
        "20",
        true,
        true,
        true,
    ));
    for spec in SPECS {
        let mut sub = common_search(
            Command::new(spec.name).about(spec.description),
            "10",
            spec.query,
            spec.country,
            spec.time,
        );
        for (name, help) in spec.extras {
            let arg = option(name, help);
            sub = sub.arg(if *name == "image-url" {
                arg.required(true)
            } else {
                arg
            });
        }
        root = root.subcommand(sub);
    }
    root = root.subcommand(
        Command::new("scrape")
            .about("Scrape a page to a Markdown file")
            .arg(option("output", "Output file name").required(true))
            .arg(option("path", "Output directory").default_value("."))
            .arg(option("url", "URL to scrape").required(true))
            .arg(
                Arg::new("include-markdown")
                    .long("include-markdown")
                    .help("Request Markdown (true or false)")
                    .num_args(0..=1)
                    .require_equals(true)
                    .default_missing_value("true")
                    .default_value("true")
                    .value_parser(["true", "false"]),
            )
            .arg(timeout_arg()),
    );
    root.subcommand(
        Command::new("credit")
            .about("Show remaining credits and QPS")
            .arg(timeout_arg()),
    )
}

fn get(matches: &ArgMatches, key: &str) -> String {
    matches
        .try_get_one::<String>(key)
        .ok()
        .flatten()
        .cloned()
        .unwrap_or_default()
}

pub fn timeout(matches: &ArgMatches) -> Result<u64, String> {
    let value = get(matches, "timeout")
        .parse::<i64>()
        .map_err(|_| "--timeout must be an integer greater than 0".to_owned())?;
    if value <= 0 {
        return Err("--timeout must be an integer greater than 0".to_owned());
    }
    if value > MAX_TIMEOUT {
        return Err("--timeout is too large".to_owned());
    }
    Ok(value as u64)
}

fn search_options(matches: &ArgMatches, name: &str) -> Result<SearchOptions, String> {
    let search_num = get(matches, "search-num")
        .parse::<u32>()
        .map_err(|_| "--search-num must be an integer from 1 to 100".to_owned())?;
    if !(1..=100).contains(&search_num) {
        return Err("--search-num must be an integer from 1 to 100".to_owned());
    }
    let query = get(matches, "query");
    if (name == "aggregated" || SPECS.iter().any(|spec| spec.name == name && spec.query))
        && query.trim().is_empty()
    {
        return Err("--query is required".to_owned());
    }
    let image_url = get(matches, "image-url");
    if name == "lens" && image_url.trim().is_empty() {
        return Err("--image-url is required".to_owned());
    }
    let options = SearchOptions {
        query,
        image_url,
        search_num,
        country: get(matches, "country"),
        language: get(matches, "language"),
        search_time: get(matches, "search-time"),
        location: get(matches, "location"),
        ll: get(matches, "ll"),
        place_id: get(matches, "place-id"),
        cid: get(matches, "cid"),
        fid: get(matches, "fid"),
        sort_by: get(matches, "sort-by"),
    };
    if name == "reviews"
        && [&options.fid, &options.cid, &options.place_id]
            .iter()
            .all(|s| s.trim().is_empty())
    {
        return Err("reviews requires at least one of --fid, --cid, --place-id".to_owned());
    }
    search::search_time(&options.search_time).map_err(str::to_owned)?;
    Ok(options)
}

pub fn execute(
    name: &str,
    matches: &ArgMatches,
    client: &SerperClient,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    if let Err(message) = timeout(matches) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    if name == "credit" {
        match client.request("account", None) {
            Ok(response) => {
                let balance = response.value.get("balance").and_then(|v| v.as_number());
                let rate_limit = response.value.get("rateLimit").and_then(|v| v.as_number());
                if let (Some(balance), Some(rate_limit)) = (balance, rate_limit) {
                    let _ = writeln!(stdout, "Timestamp      : {}\nSerper Credits : {balance}\nSerper QPS     : {rate_limit}", response.received_at.format("%Y-%m-%d %H:%M:%S"));
                    return 0;
                }
                let _ = writeln!(
                    stderr,
                    "account response must contain numeric balance and rateLimit fields"
                );
            }
            Err(error) => {
                let _ = writeln!(stderr, "{}", error.message);
            }
        }
        return 1;
    }
    if name == "scrape" {
        let output = get(matches, "output");
        let url = get(matches, "url");
        if output.trim().is_empty() || url.trim().is_empty() {
            let _ = writeln!(
                stderr,
                "{} is required",
                if output.trim().is_empty() {
                    "--output"
                } else {
                    "--url"
                }
            );
            return 2;
        }
        let directory = get(matches, "path");
        let options = ScrapeOptions {
            output: &output,
            directory: &directory,
            url: &url,
            include_markdown: get(matches, "include-markdown") == "true",
        };
        return match scrape::run(client, &options) {
            Ok(()) => {
                let _ = writeln!(stdout, "true");
                0
            }
            Err(message) => {
                let _ = writeln!(stdout, "false\n{message}");
                1
            }
        };
    }
    let options = match search_options(matches, name) {
        Ok(options) => options,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return 2;
        }
    };
    let result = if name == "aggregated" {
        search::run_aggregated(client, &options)
    } else {
        let endpoint = SPECS
            .iter()
            .find(|spec| spec.name == name)
            .expect("known subcommand")
            .endpoint;
        search::run_search(client, endpoint, &options)
    };
    match result {
        Ok(value) => {
            let _ = writeln!(stdout, "{value}");
            0
        }
        Err(error) => {
            let _ = writeln!(stdout, "{}", search::error_json(&error));
            1
        }
    }
}
