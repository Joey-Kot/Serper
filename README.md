# Serper

English | [简体中文](README_ZH.md)

Serper is a Rust command-line client for the Serper API. It searches the web, images, videos, places, reviews, news, scholarly publications, shopping results, and patents; searches by image; scrapes pages into Markdown files; and shows the account's remaining credits and QPS.

## Downloads

| Platform | Download | SHA-256 |
| --- | --- | --- |
| Linux x86_64 | [linux-amd64](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_linux_amd64.tar.gz) | [sha256](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_linux_amd64.tar.gz.sha256) |
| Linux arm64 | [linux-arm64](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_linux_arm64.tar.gz) | [sha256](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_linux_arm64.tar.gz.sha256) |
| Windows x86_64 | [windows-amd64](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_windows_amd64.zip) | [sha256](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_windows_amd64.zip.sha256) |
| Windows arm64 | [windows-arm64](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_windows_arm64.zip) | [sha256](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_windows_arm64.zip.sha256) |
| macOS x86_64 | [darwin-amd64](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_darwin_amd64.tar.gz) | [sha256](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_darwin_amd64.tar.gz.sha256) |
| macOS arm64 | [darwin-arm64](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_darwin_arm64.tar.gz) | [sha256](https://github.com/Joey-Kot/Serper/releases/download/Latest/serper_darwin_arm64.tar.gz.sha256) |

## Usage

### API key

Extract the archive and place `serper` on your `PATH`, or run it from the extracted directory. On Windows, the executable is `serper.exe`. Set `SERPER_KEY` to your Serper API key before making requests.

```bash
export SERPER_KEY="<your-serper-api-key>"
serper --help
```

In PowerShell, use `$env:SERPER_KEY = "<your-serper-api-key>"`.

### Credits and rate limit

```bash
serper credit
```

```text
Timestamp      : 2026-11-17 10:00:00
Serper Credits : 1652
Serper QPS     : 5
```

The command reads `balance` and `rateLimit` from the account endpoint. The timestamp is the local system time when the response has been received. Errors go to stderr and return a nonzero exit status. Use `--timeout` to set the request timeout in seconds.

### Search

```bash
serper aggregated --query "AI news" --search-num 20 --country US --language en --search-time day
serper general --query "AI news" --search-num 10 --search-time week
serper image --query "serper logo" --search-num 10
serper video --query "web scraping tutorial"
serper place --query "coffee" --location "Berlin,Germany"
serper maps --query "coffee" --ll "@52.5200,13.4050,14z" --search-num 20
serper reviews --cid "<google-cid>" --sort-by newest
serper news --query "OpenAI" --search-time day
serper lens --image-url "https://example.com/image.jpg"
serper scholar --query "large language models"
serper shopping --query "mechanical keyboard"
serper patents --query "battery management"
```

Search commands print compact, single-line JSON. Successful responses contain `success`, `meta`, `data`, and `credits`; request failures print a JSON error on stdout and return a nonzero status. `aggregated` combines web, news, and image results.

`--search-num` accepts `1`–`100`. It defaults to `20` for `aggregated` and `10` for other search commands. Non-image searches round up to the next multiple of 10 and fetch pages automatically; image searches request either 10 or 100 results in one request. Results are deduplicated as pages are merged. For maps queries fetching more than one page, provide `--ll`.

### Scrape pages

```bash
serper scrape --output example --url "https://www.example.com"
serper scrape --output example --path ./exports --url "https://www.example.com" --timeout 120
```

The command creates the output directory if needed and writes `<output>.md` after a successful request. It prints `true` on success. On failure it prints `false` followed by the reason and exits with a nonzero status. Existing files and symlinks are not overwritten. Markdown is requested by default; set `--include-markdown=false` to disable it.

```markdown
## title: ...
## description: ...
## url: ...
## credits: ...

---

page content
```

### Command and option reference

| Command | Purpose |
| --- | --- |
| `aggregated` | Search the web, news, and images together. |
| `general`, `image`, `video`, `news` | Search one result type. |
| `place`, `maps`, `reviews` | Search places, maps, or reviews. |
| `lens` | Search using a public image URL. |
| `scholar`, `shopping`, `patents` | Search scholarly publications, shopping results, or patents. |
| `scrape` | Save a page as Markdown. |
| `credit` | Show remaining credits and the account QPS. |

| Option | Applies to | Usage |
| --- | --- | --- |
| `--query TEXT` | Search commands except `reviews` and `lens` | Search keywords; required for these commands. |
| `--search-num N` | Search commands | Results requested, from `1` to `100`; subject to the normalization above. |
| `--country COUNTRY` | Search commands except `patents` | Country name or ISO code; defaults to `US`. Unrecognized names fall back to `US`. |
| `--language CODE` | Search commands except `patents` | Language code, such as `en`. |
| `--search-time TIME` | `aggregated`, `general`, `image`, `video`, `news` | `hour`, `day`, `week`, `month`, or `year`. |
| `--location LOCATION` | `place` | Location hint. |
| `--ll COORDINATES` | `maps` | Map coordinates; required for multi-page query results. |
| `--place-id ID` | `maps`, `reviews` | Google place ID. |
| `--cid CID` | `maps`, `reviews` | Google CID. |
| `--fid FID` | `reviews` | Google FID. |
| `--sort-by ORDER` | `reviews` | Review sort option, such as `newest`. |
| `--image-url URL` | `lens` | Public image URL; required. |
| `--output NAME` | `scrape` | Output file name; required. The `.md` extension is added if absent. |
| `--path DIR` | `scrape` | Output directory; defaults to the current directory. |
| `--url URL` | `scrape` | Page URL; required. |
| `--include-markdown[=true\|false]` | `scrape` | Request Markdown; defaults to `true`. |
| `--timeout SECONDS` | All commands | Timeout per request; a positive integer, default `120`. |
| `--help` | All commands | Show help for a command. |

`reviews` requires at least one of `--fid`, `--cid`, or `--place-id`. Use `serper <command> --help` to see the options for an individual command. Values can be passed as `--option VALUE` or `--option=VALUE`.

## Build from source

Use Rust 1.99.1 and a C toolchain for the target platform. Rustup selects the version from [rust-toolchain.toml](rust-toolchain.toml).

```bash
git clone https://github.com/Joey-Kot/Serper.git
cd Serper
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy
cargo build --release --locked
```

The executable is `target/release/serper` (`serper.exe` on Windows). For a statically linked Linux x86_64 executable, install `musl-gcc` (the `musl-tools` package on Ubuntu) and use:

```bash
rustup target add --toolchain 1.99.1 x86_64-unknown-linux-musl
cargo build --release --locked --target x86_64-unknown-linux-musl
```

The executable is `target/x86_64-unknown-linux-musl/release/serper`. On Linux arm64, use `aarch64-unknown-linux-musl` instead. To install the Linux static build on your Cargo binary path, run `cargo install --path . --locked --target x86_64-unknown-linux-musl` with the appropriate target.

Run the project checks with:

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

## License

Serper is licensed under the MIT License. [License text](LICENSE)
