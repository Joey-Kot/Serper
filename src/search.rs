use crate::api::{ApiError, SerperClient};
use crate::country::country_code;
use serde_json::{json, Map, Value};
use std::collections::HashSet;

#[derive(Default)]
pub struct SearchOptions {
    pub query: String,
    pub image_url: String,
    pub search_num: u32,
    pub country: String,
    pub language: String,
    pub search_time: String,
    pub location: String,
    pub ll: String,
    pub place_id: String,
    pub cid: String,
    pub fid: String,
    pub sort_by: String,
}

pub fn search_time(value: &str) -> Result<&'static str, &'static str> {
    match value.trim().to_lowercase().as_str() {
        "" => Ok(""),
        "hour" | "h" | "qdr:h" => Ok("qdr:h"),
        "day" | "d" | "qdr:d" => Ok("qdr:d"),
        "week" | "w" | "qdr:w" => Ok("qdr:w"),
        "month" | "m" | "qdr:m" => Ok("qdr:m"),
        "year" | "y" | "qdr:y" => Ok("qdr:y"),
        _ => {
            Err("--search-time must be one of hour (h), day (d), week (w), month (m), or year (y)")
        }
    }
}

fn payload(endpoint: &str, options: &SearchOptions) -> Value {
    let mut result = Map::new();
    let mut put = |key: &str, value: &str| {
        if !value.is_empty() {
            result.insert(key.to_owned(), json!(value));
        }
    };
    put("q", &options.query);
    if endpoint == "lens" {
        put("url", &options.image_url);
    }
    if endpoint != "patents" {
        put("gl", &country_code(&options.country));
        put("hl", &options.language);
    }
    if matches!(endpoint, "search" | "images" | "videos" | "news") {
        put("tbs", search_time(&options.search_time).unwrap_or(""));
    }
    for (key, value) in [
        ("location", &options.location),
        ("ll", &options.ll),
        ("placeId", &options.place_id),
        ("cid", &options.cid),
        ("fid", &options.fid),
        ("sortBy", &options.sort_by),
    ] {
        put(key, value);
    }
    Value::Object(result)
}

fn items_key(endpoint: &str) -> &'static str {
    match endpoint {
        "search" | "lens" | "scholar" | "patents" => "organic",
        "images" => "images",
        "videos" => "videos",
        "places" | "maps" => "places",
        "reviews" => "reviews",
        "news" => "news",
        "shopping" => "shopping",
        _ => unreachable!("unknown search endpoint"),
    }
}

fn effective_num(endpoint: &str, requested: u32) -> u32 {
    let requested = requested.clamp(1, 100);
    if endpoint == "images" {
        if requested <= 10 {
            10
        } else {
            100
        }
    } else {
        requested.div_ceil(10) * 10
    }
}

fn integer(value: &Value) -> i64 {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|v| i64::try_from(v).ok()))
        .or_else(|| value.as_f64().map(|v| v as i64))
        .unwrap_or(0)
}

fn string_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn unique_key(item: &Value) -> String {
    for key in [
        "id",
        "link",
        "cid",
        "placeId",
        "publicationNumber",
        "productId",
        "title",
    ] {
        let value = string_value(&item[key]);
        if !value.trim().is_empty() {
            return format!("{key}:{}", value.trim());
        }
    }
    item.to_string()
}

fn fetch_pages(
    client: &SerperClient,
    endpoint: &str,
    options: &SearchOptions,
) -> Result<(Value, Value), ApiError> {
    let effective = effective_num(endpoint, options.search_num);
    let pages = if endpoint == "images" {
        1
    } else {
        effective / 10
    };
    let base = payload(endpoint, options);
    if endpoint == "maps"
        && pages > 1
        && !options.query.trim().is_empty()
        && options.ll.trim().is_empty()
    {
        return Err(ApiError {
            message: "maps multi-page aggregation requires --ll in query mode".to_owned(),
            status_code: Some(400),
        });
    }
    let key = items_key(endpoint);
    let mut all = Vec::new();
    let mut seen = HashSet::new();
    let mut credits: i64 = 0;
    let mut first = Value::Null;
    for page in 1..=pages {
        let mut request = base.clone();
        request["page"] = json!(page);
        if endpoint == "images" {
            request["num"] = json!(effective);
        }
        let result = client.request(endpoint, Some(&request))?.value;
        credits = credits.saturating_add(integer(&result["credits"]));
        if let Some(items) = result[key].as_array() {
            for item in items.iter().filter(|v| v.is_object()) {
                if seen.insert(unique_key(item)) && all.len() < effective as usize {
                    all.push(item.clone());
                }
            }
        }
        if page == 1 {
            first = result;
        }
    }
    let mut merged = json!({key: all, "credits": credits});
    if endpoint == "search" {
        merged["knowledgeGraph"] = first["knowledgeGraph"].clone();
        merged["peopleAlsoAsk"] = first
            .get("peopleAlsoAsk")
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or_else(|| json!([]));
        merged["relatedSearches"] = first
            .get("relatedSearches")
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or_else(|| json!([]));
    }
    if endpoint == "maps" {
        merged["ll"] = first["ll"].clone();
    }
    let meta = json!({
        "requested_search_num": options.search_num,
        "effective_search_num": effective,
        "pages_fetched": pages,
        "result_count": merged[key].as_array().map_or(0, Vec::len),
        "credits": credits,
    });
    Ok((merged, meta))
}

fn mapped_items(raw: &Value, key: &str, fields: &[&str]) -> Value {
    let items = raw[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item.is_object())
        .map(|item| {
            let values: Map<String, Value> = fields
                .iter()
                .map(|field| ((*field).to_owned(), item[*field].clone()))
                .collect();
            Value::Object(values)
        })
        .collect::<Vec<_>>();
    json!(items)
}

fn transform(endpoint: &str, raw: &Value) -> Value {
    let (key, fields): (&str, &[&str]) = match endpoint {
        "search" => ("organic", &["title", "link", "snippet", "date", "position"]),
        "images" => (
            "images",
            &[
                "title",
                "link",
                "imageUrl",
                "thumbnailUrl",
                "source",
                "position",
            ],
        ),
        "videos" => (
            "videos",
            &[
                "title", "link", "snippet", "source", "channel", "duration", "date", "imageUrl",
                "position",
            ],
        ),
        "places" => (
            "places",
            &[
                "title",
                "address",
                "phoneNumber",
                "website",
                "latitude",
                "longitude",
                "cid",
                "position",
            ],
        ),
        "maps" => (
            "places",
            &[
                "title",
                "address",
                "rating",
                "ratingCount",
                "type",
                "website",
                "phoneNumber",
                "latitude",
                "longitude",
                "cid",
                "fid",
                "placeId",
                "position",
            ],
        ),
        "news" => (
            "news",
            &["title", "link", "snippet", "date", "source", "imageUrl"],
        ),
        "lens" => (
            "organic",
            &["title", "link", "source", "imageUrl", "thumbnailUrl"],
        ),
        "scholar" => (
            "organic",
            &[
                "title",
                "link",
                "publicationInfo",
                "snippet",
                "year",
                "citedBy",
                "pdfUrl",
                "id",
            ],
        ),
        "shopping" => (
            "shopping",
            &[
                "title",
                "source",
                "link",
                "price",
                "rating",
                "ratingCount",
                "productId",
                "position",
            ],
        ),
        "patents" => (
            "organic",
            &[
                "title",
                "link",
                "snippet",
                "priorityDate",
                "filingDate",
                "grantDate",
                "publicationDate",
                "inventor",
                "assignee",
                "publicationNumber",
                "pdfUrl",
            ],
        ),
        "reviews" => {
            let mut reviews = mapped_items(
                raw,
                "reviews",
                &["rating", "date", "isoDate", "snippet", "id", "user"],
            );
            for review in reviews.as_array_mut().unwrap() {
                let user = review["user"].clone();
                review["user"] = json!({"name": user.get("name"), "link": user.get("link"), "reviews": user.get("reviews"), "photos": user.get("photos")});
            }
            return json!({"reviews": reviews});
        }
        _ => unreachable!("unknown search endpoint"),
    };
    let mut result = json!({key: mapped_items(raw, key, fields)});
    if endpoint == "search" {
        let kg = raw
            .get("knowledgeGraph")
            .filter(|v| v.is_object())
            .cloned()
            .unwrap_or_else(|| json!({}));
        result["knowledge_graph"] = json!({"title": kg["title"], "description": kg["description"], "descriptionLink": kg["descriptionLink"], "imageUrl": kg["imageUrl"]});
        result["people_also_ask"] = mapped_items(
            raw,
            "peopleAlsoAsk",
            &["question", "title", "link", "snippet"],
        );
        result["related_searches"] = mapped_items(raw, "relatedSearches", &["query"]);
    }
    if endpoint == "maps" {
        result["ll"] = raw["ll"].clone();
    }
    result
}

pub fn run_search(
    client: &SerperClient,
    endpoint: &str,
    options: &SearchOptions,
) -> Result<Value, ApiError> {
    let (raw, meta) = fetch_pages(client, endpoint, options)?;
    Ok(
        json!({"success": true, "credits": raw["credits"], "meta": meta, "data": transform(endpoint, &raw)}),
    )
}

pub fn run_aggregated(client: &SerperClient, options: &SearchOptions) -> Result<Value, ApiError> {
    let (web, web_meta) = fetch_pages(client, "search", options)?;
    let (news, news_meta) = fetch_pages(client, "news", options)?;
    let (images, images_meta) = fetch_pages(client, "images", options)?;
    let data = json!({"web": transform("search", &web)["organic"], "news": transform("news", &news)["news"], "images": transform("images", &images)["images"]});
    let credits = integer(&web["credits"])
        .saturating_add(integer(&news["credits"]))
        .saturating_add(integer(&images["credits"]));
    let meta = json!({
        "requested_search_num": options.search_num,
        "effective_web_search_num": web_meta["effective_search_num"],
        "effective_news_search_num": news_meta["effective_search_num"],
        "effective_image_search_num": images_meta["effective_search_num"],
        "pages_fetched": {"web": web_meta["pages_fetched"], "news": news_meta["pages_fetched"], "images": images_meta["pages_fetched"]},
        "result_count": {"web": data["web"].as_array().unwrap().len(), "news": data["news"].as_array().unwrap().len(), "images": data["images"].as_array().unwrap().len()},
    });
    Ok(json!({"success": true, "meta": meta, "data": data, "credits": credits}))
}

pub fn error_json(error: &ApiError) -> Value {
    let mut value = json!({"success": false, "error": true, "message": error.message});
    if let Some(status) = error.status_code.filter(|s| *s > 0) {
        value["status_code"] = json!(status);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_search_counts() {
        assert_eq!(effective_num("search", 25), 30);
        assert_eq!(effective_num("images", 10), 10);
        assert_eq!(effective_num("images", 11), 100);
    }

    #[test]
    fn maps_fields_and_drops_unrequested_fields() {
        let raw =
            json!({"shopping":[{"title":"p", "imageUrl":"large", "rating":4.9}], "credits":1});
        let result = transform("shopping", &raw);
        assert_eq!(result["shopping"][0]["title"], "p");
        assert!(result["shopping"][0].get("imageUrl").is_none());
        assert_eq!(result["shopping"][0]["link"], Value::Null);
        let reviews = transform(
            "reviews",
            &json!({"reviews": [{"id":"1", "user": {"name":"n", "extra":"drop"}}]}),
        );
        assert_eq!(reviews["reviews"][0]["user"]["name"], "n");
        assert!(reviews["reviews"][0]["user"].get("extra").is_none());
    }
}
