use crate::{api::SerperClient, cli};
use chrono::Local;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

#[cfg(all(target_os = "windows", target_env = "msvc"))]
#[test]
fn windows_uses_static_c_runtime() {
    assert!(cfg!(target_feature = "crt-static"));
}

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    key: Option<String>,
    content_type: Option<String>,
    body: Vec<u8>,
}

fn mock_server(
    replies: Vec<(u16, &'static str, String)>,
) -> (String, Receiver<Request>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        for (status, content_type, body) in replies {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let parts: Vec<_> = line.split_whitespace().collect();
            let method = parts[0].to_owned();
            let path = parts[1].to_owned();
            let mut key = None;
            let mut request_content_type = None;
            let mut length = 0;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                if let Some((name, value)) = line.trim_end().split_once(':') {
                    if name.eq_ignore_ascii_case("x-api-key") {
                        key = Some(value.trim().to_owned());
                    }
                    if name.eq_ignore_ascii_case("content-type") {
                        request_content_type = Some(value.trim().to_owned());
                    }
                    if name.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse().unwrap();
                    }
                }
            }
            let mut request_body = vec![0; length];
            reader.read_exact(&mut request_body).unwrap();
            sender
                .send(Request {
                    method,
                    path,
                    key,
                    content_type: request_content_type,
                    body: request_body,
                })
                .unwrap();
            write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    (url, receiver, handle)
}

fn reply(status: u16, value: Value) -> (u16, &'static str, String) {
    (status, "application/json", value.to_string())
}

fn invoke(url: &str, args: &[&str]) -> (i32, String, String) {
    let matches = cli::command()
        .try_get_matches_from(std::iter::once("serper").chain(args.iter().copied()))
        .unwrap();
    let (name, sub) = matches.subcommand().unwrap();
    let client = SerperClient::new(" test-key ".to_owned(), 5)
        .unwrap()
        .with_test_server(url);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = cli::execute(name, sub, &client, &mut stdout, &mut stderr);
    (
        code,
        String::from_utf8(stdout).unwrap(),
        String::from_utf8(stderr).unwrap(),
    )
}

fn body(request: &Request) -> Value {
    serde_json::from_slice(&request.body).unwrap()
}

#[test]
fn credit_gets_account_and_prints_values_with_response_time() {
    let (url, receiver, handle) =
        mock_server(vec![reply(200, json!({"balance":1652,"rateLimit":5}))]);
    let before = Local::now().timestamp();
    let (code, out, err) = invoke(&url, &["credit"]);
    let after = Local::now().timestamp();
    let request = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/account");
    assert_eq!(request.key.as_deref(), Some("test-key"));
    assert!(request.body.is_empty());
    let lines: Vec<_> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    let timestamp = lines[0].strip_prefix("Timestamp      : ").unwrap();
    let local = chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S").unwrap();
    let observed = chrono::TimeZone::from_local_datetime(&Local, &local)
        .earliest()
        .unwrap()
        .timestamp();
    assert!((before..=after).contains(&observed));
    assert_eq!(lines[1], "Serper Credits : 1652");
    assert_eq!(lines[2], "Serper QPS     : 5");
}

#[test]
fn credit_rejects_missing_or_non_numeric_fields_and_http_errors() {
    for payload in [
        json!({"balance":1}),
        json!({"balance":"1652","rateLimit":5}),
        json!({"balance":1,"rateLimit":null}),
    ] {
        let (url, receiver, handle) = mock_server(vec![reply(200, payload)]);
        let (code, out, err) = invoke(&url, &["credit"]);
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        handle.join().unwrap();
        assert_eq!(code, 1);
        assert!(out.is_empty());
        assert!(err.contains("balance and rateLimit"));
    }
    let (url, receiver, handle) =
        mock_server(vec![reply(401, json!({"message":"Invalid API key"}))]);
    let (code, out, err) = invoke(&url, &["credit"]);
    receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    handle.join().unwrap();
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("401") && err.contains("Invalid API key"));
}

#[test]
fn web_paginates_deduplicates_and_maps_fields() {
    let (url, receiver, handle) = mock_server(vec![
        reply(
            200,
            json!({"organic":[{"title":"first","link":"a","extra":"drop"},{"title":"duplicate","link":"dup"}], "knowledgeGraph":{"title":"kg"},"credits":1}),
        ),
        reply(
            200,
            json!({"organic":[{"title":"duplicate","link":"dup"},{"title":"second","link":"b"}],"credits":2}),
        ),
    ]);
    let (code, out, err) = invoke(
        &url,
        &[
            "web",
            "--query",
            "ai",
            "--country",
            "United Kingdom",
            "--language",
            "en",
            "--search-num",
            "15",
            "--search-time",
            "month",
        ],
    );
    let first = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    let second = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert_eq!(first.method, "POST");
    assert_eq!(first.path, "/search");
    assert_eq!(first.key.as_deref(), Some("test-key"));
    assert_eq!(first.content_type.as_deref(), Some("application/json"));
    assert_eq!(
        body(&first),
        json!({"q":"ai", "gl":"GB", "hl":"en", "tbs":"qdr:m", "page":1})
    );
    assert_eq!(body(&second)["page"], 2);
    let result: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(result["meta"]["effective_search_num"], 20);
    assert_eq!(result["meta"]["pages_fetched"], 2);
    assert_eq!(result["meta"]["result_count"], 3);
    assert_eq!(result["credits"], 3);
    assert_eq!(result["data"]["knowledge_graph"]["title"], "kg");
    assert!(result["data"]["organic"][0].get("extra").is_none());
    assert_eq!(out.lines().count(), 1);
}

#[test]
fn image_and_aggregated_search_use_endpoint_specific_pagination() {
    let (url, receiver, handle) = mock_server(vec![reply(
        200,
        json!({"images":[{"title":"a","link":"a"}],"credits":2}),
    )]);
    let (code, out, _) = invoke(&url, &["image", "--query", "logo", "--search-num", "11"]);
    let request = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    handle.join().unwrap();
    assert_eq!(code, 0);
    assert_eq!(request.path, "/images");
    assert_eq!(body(&request)["num"], 100);
    assert_eq!(body(&request)["page"], 1);
    let value: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(value["meta"]["effective_search_num"], 100);

    let (url, receiver, handle) = mock_server(vec![
        reply(200, json!({"organic":[{"link":"web1"}],"credits":1})),
        reply(200, json!({"organic":[{"link":"web2"}],"credits":1})),
        reply(200, json!({"news":[{"link":"news1"}],"credits":1})),
        reply(200, json!({"news":[{"link":"news2"}],"credits":1})),
        reply(200, json!({"images":[{"link":"image1"}],"credits":2})),
    ]);
    let (code, out, _) = invoke(&url, &["aggregated", "--query", "ai"]);
    let requests: Vec<_> = (0..5)
        .map(|_| receiver.recv_timeout(Duration::from_secs(5)).unwrap())
        .collect();
    handle.join().unwrap();
    assert_eq!(code, 0);
    assert_eq!(
        requests.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
        ["/search", "/search", "/news", "/news", "/images"]
    );
    assert_eq!(body(&requests[4])["num"], 100);
    let result: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(result["credits"], 6);
    assert_eq!(
        result["meta"]["pages_fetched"],
        json!({"web":2, "news":2, "images":1})
    );
    assert_eq!(
        result["meta"]["result_count"],
        json!({"web":2, "news":2, "images":1})
    );
}

#[test]
fn scrape_writes_markdown_and_does_not_overwrite_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested");
    let path_str = path.to_str().unwrap();
    let (url, receiver, handle) = mock_server(vec![reply(
        200,
        json!({"metadata":{"title":"T","description":"D","og:url":"https://metadata.example/"},"markdown":"hello%20world\\nnext","credits":3}),
    )]);
    let (code, out, err) = invoke(
        &url,
        &[
            "scrape",
            "--output",
            "page",
            "--path",
            path_str,
            "--url",
            "https://example.com",
            "--include-markdown=false",
        ],
    );
    let request = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert_eq!(out, "true\n");
    assert_eq!(request.path, "/scrape");
    assert_eq!(body(&request), json!({"url":"https://example.com"}));
    let file = path.join("page.md");
    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.contains(
        "## title: T\n## description: D\n## url: https://metadata.example/\n## credits: 3"
    ));
    assert!(content.contains("hello world\nnext"));
    let (code, out, _) = invoke(
        "http://127.0.0.1:1",
        &[
            "scrape",
            "--output",
            "page",
            "--path",
            path_str,
            "--url",
            "https://example.com",
        ],
    );
    assert_eq!(code, 1);
    assert!(out.starts_with("false\noutput file already exists:"));
    assert_eq!(std::fs::read_to_string(file).unwrap(), content);
}

#[cfg(unix)]
#[test]
fn scrape_rejects_dangling_symlink_before_network_request() {
    let dir = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(dir.path().join("missing"), dir.path().join("page.md")).unwrap();
    let (code, out, _) = invoke(
        "http://127.0.0.1:1",
        &[
            "scrape",
            "--output",
            "page",
            "--path",
            dir.path().to_str().unwrap(),
            "--url",
            "https://example.com",
        ],
    );
    assert_eq!(code, 1);
    assert!(out.contains("output file already exists"));
}

#[test]
fn search_and_scrape_errors_are_visible_and_do_not_create_file() {
    let (url, receiver, handle) =
        mock_server(vec![(502, "text/html", "upstream gateway".to_owned())]);
    let (code, out, _) = invoke(&url, &["web", "--query", "a"]);
    receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    handle.join().unwrap();
    assert_eq!(code, 1);
    let value: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(value["status_code"], 502);
    assert!(value["message"]
        .as_str()
        .unwrap()
        .contains("upstream gateway"));

    let dir = tempfile::tempdir().unwrap();
    let (url, receiver, handle) = mock_server(vec![(200, "text/plain", "not json".to_owned())]);
    let (code, out, _) = invoke(
        &url,
        &[
            "scrape",
            "--output",
            "page",
            "--path",
            dir.path().to_str().unwrap(),
            "--url",
            "https://example.com",
        ],
    );
    receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    handle.join().unwrap();
    assert_eq!(code, 1);
    assert!(out.contains("false\nscrape response JSON parse failed"));
    assert!(out.contains("status=200") && out.contains("not json"));
    assert!(!dir.path().join("page.md").exists());
}

#[test]
fn input_validation_happens_before_network_requests() {
    for args in [
        vec!["reviews"],
        vec!["web", "--query", "test", "--timeout", "0"],
        vec!["web", "--query", "test", "--search-num", "101"],
        vec!["web", "--query", "test", "--search-time", "invalid"],
    ] {
        let (code, _, err) = invoke("http://127.0.0.1:1", &args);
        assert_eq!(code, 2, "{args:?} {err}");
    }
    let (code, out, _) = invoke(
        "http://127.0.0.1:1",
        &["maps", "--query", "coffee", "--search-num", "25"],
    );
    assert_eq!(code, 1);
    let value: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(value["status_code"], 400);
    assert!(value["message"].as_str().unwrap().contains("--ll"));
}

#[test]
fn each_search_command_uses_the_correct_endpoint_payload_and_schema() {
    struct Case<'a> {
        args: Vec<&'a str>,
        endpoint: &'a str,
        item_key: &'a str,
        extra_field: &'a str,
        extra_value: Value,
    }
    let cases = vec![
        Case {
            args: vec!["video", "--query", "x", "--search-time", "day"],
            endpoint: "videos",
            item_key: "videos",
            extra_field: "tbs",
            extra_value: json!("qdr:d"),
        },
        Case {
            args: vec!["place", "--query", "x", "--location", "Berlin"],
            endpoint: "places",
            item_key: "places",
            extra_field: "location",
            extra_value: json!("Berlin"),
        },
        Case {
            args: vec![
                "maps",
                "--query",
                "x",
                "--ll",
                "@1,2,14z",
                "--place-id",
                "id",
            ],
            endpoint: "maps",
            item_key: "places",
            extra_field: "placeId",
            extra_value: json!("id"),
        },
        Case {
            args: vec!["reviews", "--cid", "123", "--sort-by", "newest"],
            endpoint: "reviews",
            item_key: "reviews",
            extra_field: "sortBy",
            extra_value: json!("newest"),
        },
        Case {
            args: vec!["news", "--query", "x", "--search-time", "week"],
            endpoint: "news",
            item_key: "news",
            extra_field: "tbs",
            extra_value: json!("qdr:w"),
        },
        Case {
            args: vec!["lens", "--image-url", "https://example.com/a.jpg"],
            endpoint: "lens",
            item_key: "organic",
            extra_field: "url",
            extra_value: json!("https://example.com/a.jpg"),
        },
        Case {
            args: vec!["scholar", "--query", "x", "--language", "en"],
            endpoint: "scholar",
            item_key: "organic",
            extra_field: "hl",
            extra_value: json!("en"),
        },
        Case {
            args: vec!["shopping", "--query", "x", "--country", "CN"],
            endpoint: "shopping",
            item_key: "shopping",
            extra_field: "gl",
            extra_value: json!("CN"),
        },
        Case {
            args: vec!["patents", "--query", "x"],
            endpoint: "patents",
            item_key: "organic",
            extra_field: "q",
            extra_value: json!("x"),
        },
    ];
    for case in cases {
        let reply_value = json!({case.item_key:[{"title":"result", "id":"test-id", "user":{"name":"N"}, "extra":"drop"}], "credits":1});
        let (url, receiver, handle) = mock_server(vec![reply(200, reply_value)]);
        let (code, out, err) = invoke(&url, &case.args);
        let request = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        handle.join().unwrap();
        assert_eq!(code, 0, "{:?}: {err}", case.args);
        assert_eq!(request.path, format!("/{}", case.endpoint));
        let sent = body(&request);
        assert_eq!(sent[case.extra_field], case.extra_value, "{:?}", case.args);
        assert_eq!(sent["page"], 1);
        if case.endpoint == "patents" {
            assert!(sent.get("gl").is_none());
        }
        let result: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(result["meta"]["result_count"], 1, "{:?}", case.args);
        assert_eq!(result["credits"], 1);
        assert!(result["data"][case.item_key][0].get("extra").is_none());
        if case.endpoint == "reviews" {
            assert_eq!(result["data"]["reviews"][0]["user"]["name"], "N");
        } else {
            assert_eq!(result["data"][case.item_key][0]["title"], "result");
        }
    }
}

#[test]
fn empty_key_fails_for_credit_without_sending_request() {
    let matches = cli::command()
        .try_get_matches_from(["serper", "credit"])
        .unwrap();
    let (_, sub) = matches.subcommand().unwrap();
    let client = SerperClient::new("   ".to_owned(), 5)
        .unwrap()
        .with_test_server("http://127.0.0.1:1");
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(cli::execute("credit", sub, &client, &mut out, &mut err), 1);
    assert!(out.is_empty());
    assert_eq!(String::from_utf8(err).unwrap(), "SERPER_KEY is required\n");
}

#[test]
fn credit_records_time_after_complete_response() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
        }
        let body = r#"{"balance":0,"rateLimit":5}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.flush().unwrap();
        thread::sleep(Duration::from_millis(1100));
        let sent_at = Local::now().timestamp();
        write!(stream, "{body}").unwrap();
        sender.send(sent_at).unwrap();
    });
    let (code, out, err) = invoke(&url, &["credit"]);
    let sent_at = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let timestamp = out
        .lines()
        .next()
        .unwrap()
        .strip_prefix("Timestamp      : ")
        .unwrap();
    let parsed = chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S").unwrap();
    let observed = chrono::TimeZone::from_local_datetime(&Local, &parsed)
        .earliest()
        .unwrap()
        .timestamp();
    assert!(
        observed >= sent_at,
        "timestamp {observed} preceded response body {sent_at}"
    );
    assert!(out.contains("Serper Credits : 0\n"));
}

#[test]
fn scrape_default_requests_markdown_and_rejects_invalid_filename() {
    let dir = tempfile::tempdir().unwrap();
    let (url, receiver, handle) =
        mock_server(vec![reply(200, json!({"markdown":"body","credits":1}))]);
    let (code, out, _) = invoke(
        &url,
        &[
            "scrape",
            "--output",
            "page",
            "--path",
            dir.path().to_str().unwrap(),
            "--url",
            "https://example.com",
        ],
    );
    let request = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    handle.join().unwrap();
    assert_eq!(code, 0);
    assert_eq!(out, "true\n");
    assert_eq!(body(&request)["includeMarkdown"], true);
    let content = std::fs::read_to_string(dir.path().join("page.md")).unwrap();
    assert!(content.contains("## url: https://example.com"));

    let (code, out, _) = invoke(
        "http://127.0.0.1:1",
        &[
            "scrape",
            "--output",
            "/",
            "--path",
            dir.path().to_str().unwrap(),
            "--url",
            "https://example.com",
        ],
    );
    assert_eq!(code, 1);
    assert!(out.contains("--output must contain a file name"));
}

#[test]
fn search_accepts_query_starting_with_hyphen() {
    let (url, receiver, handle) = mock_server(vec![reply(200, json!({"organic":[],"credits":1}))]);
    let (code, _, err) = invoke(&url, &["web", "--query", "-term"]);
    let request = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    handle.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert_eq!(body(&request)["q"], "-term");
}

#[test]
fn request_timeout_applies_while_waiting_for_response_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
        }
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n").unwrap();
        stream.flush().unwrap();
        thread::sleep(Duration::from_millis(1500));
    });
    let matches = cli::command()
        .try_get_matches_from(["serper", "credit", "--timeout", "1"])
        .unwrap();
    let (name, sub) = matches.subcommand().unwrap();
    let client = SerperClient::new("test-key".to_owned(), cli::timeout(sub).unwrap())
        .unwrap()
        .with_test_server(&url);
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(cli::execute(name, sub, &client, &mut out, &mut err), 1);
    assert!(out.is_empty());
    assert!(String::from_utf8(err)
        .unwrap()
        .contains("response read failed"));
    handle.join().unwrap();
}
