use crate::api::SerperClient;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

pub struct ScrapeOptions<'a> {
    pub output: &'a str,
    pub directory: &'a str,
    pub url: &'a str,
    pub include_markdown: bool,
}

fn display(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn output_path(output: &str, directory: &str) -> Result<PathBuf, String> {
    let trimmed = output.trim();
    let mut name = Path::new(trimmed)
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "--output must contain a file name".to_owned())?
        .to_owned();
    if !name.to_lowercase().ends_with(".md") {
        name.push_str(".md");
    }
    let dir = if directory.trim().is_empty() {
        "."
    } else {
        directory.trim()
    };
    Ok(Path::new(dir).join(name))
}

fn decode_markdown(input: &str) -> String {
    // Go's PathUnescape leaves the entire value unchanged on an invalid escape.
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit()
            {
                return input.replace("\\n", "\n");
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    let decoded = percent_encoding::percent_decode_str(input)
        .decode_utf8()
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| input.to_owned());
    decoded.replace("\\n", "\n")
}

fn render_markdown(raw: &Value, url: &str) -> String {
    let metadata = &raw["metadata"];
    let title = display(&metadata["title"]);
    let description = [
        display(&metadata["description"]),
        display(&metadata["og:description"]),
    ]
    .into_iter()
    .find(|s| !s.trim().is_empty())
    .unwrap_or_default();
    let source_url = [display(&metadata["og:url"]), url.to_owned()]
        .into_iter()
        .find(|s| !s.trim().is_empty())
        .unwrap_or_default();
    let credits = display(&raw["credits"]);
    let markdown = decode_markdown(&display(&raw["markdown"]));
    format!("## title: {title}\n## description: {description}\n## url: {source_url}\n## credits: {credits}\n\n---\n\n{markdown}\n")
}

pub fn run(client: &SerperClient, options: &ScrapeOptions<'_>) -> Result<(), String> {
    let directory = if options.directory.trim().is_empty() {
        "."
    } else {
        options.directory.trim()
    };
    fs::create_dir_all(directory)
        .map_err(|err| format!("cannot create output directory: {err}"))?;
    let path = output_path(options.output, directory)?;
    match fs::symlink_metadata(&path) {
        Ok(_) => return Err(format!("output file already exists: {}", path.display())),
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(err.to_string()),
    }
    let mut request = json!({"url": options.url});
    if options.include_markdown {
        request["includeMarkdown"] = json!(true);
    }
    let response = client
        .request("scrape", Some(&request))
        .map_err(|err| err.message)?;
    let content = render_markdown(&response.value, options.url);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|err| {
            if err.kind() == ErrorKind::AlreadyExists {
                format!("output file already exists: {}", path.display())
            } else {
                err.to_string()
            }
        })?;
    if let Err(err) = file
        .write_all(content.as_bytes())
        .and_then(|()| file.flush())
    {
        drop(file);
        let _ = fs::remove_file(&path);
        return Err(err.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_decoding_and_metadata_fallback() {
        let raw = json!({"metadata":{"title":"T", "og:description":"D"},"markdown":"hello%20world\\nnext", "credits":3});
        let result = render_markdown(&raw, "https://input.example");
        assert!(result.contains(
            "## title: T\n## description: D\n## url: https://input.example\n## credits: 3"
        ));
        assert!(result.contains("hello world\nnext"));
        assert_eq!(decode_markdown("bad%20escape%z"), "bad%20escape%z");
    }
}
