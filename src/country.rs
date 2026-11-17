use std::collections::HashMap;
use std::sync::OnceLock;

static ALIASES: OnceLock<HashMap<String, String>> = OnceLock::new();

fn normalize(value: &str) -> String {
    let mut result = String::new();
    let mut previous_space = true;
    for c in value.to_lowercase().chars() {
        if c.is_alphanumeric() || c == '\'' || c == '-' {
            result.push(c);
            previous_space = false;
        } else if !previous_space {
            result.push(' ');
            previous_space = true;
        }
    }
    result.trim().to_owned()
}

fn aliases() -> &'static HashMap<String, String> {
    ALIASES.get_or_init(|| {
        let countries: HashMap<String, Vec<String>> =
            serde_json::from_str(include_str!("../data/country_aliases.json"))
                .expect("embedded country aliases must be valid");
        let mut result = HashMap::new();
        for (code, names) in countries {
            for name in names {
                let base = name.trim();
                let normalized = normalize(base);
                let stripped: String = normalized
                    .chars()
                    .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                    .collect();
                let mut variants = vec![base.to_owned(), normalized.clone(), stripped];
                let comma_parts: Vec<_> = base
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();
                if comma_parts.len() >= 2 {
                    variants.push(comma_parts.into_iter().rev().collect::<Vec<_>>().join(" "));
                }
                let parts: Vec<_> = normalized.split_whitespace().collect();
                if parts.len() == 2 {
                    variants.push(format!("{} {}", parts[1], parts[0]));
                }
                for variant in variants {
                    let key = normalize(&variant);
                    if !key.is_empty() {
                        result.insert(key, code.to_uppercase());
                    }
                }
            }
        }
        result
    })
}

pub fn country_code(input: &str) -> String {
    let name = input.trim();
    if let Some(code) = aliases().get(&normalize(name)) {
        return code.clone();
    }
    if name.chars().count() == 2 && name.chars().all(char::is_alphabetic) {
        return name.to_uppercase();
    }
    "US".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_aliases_and_fallbacks() {
        for (name, expected) in [
            ("U.S.", "US"),
            ("United Kingdom", "GB"),
            ("中国", "CN"),
            ("China", "CN"),
            ("unknownland", "US"),
            ("", "US"),
        ] {
            assert_eq!(country_code(name), expected, "{name}");
        }
    }

    #[test]
    fn embedded_data_has_valid_country_codes_and_names() {
        let data: HashMap<String, Vec<String>> =
            serde_json::from_str(include_str!("../data/country_aliases.json")).unwrap();
        assert!(data.len() > 200);
        for (code, aliases) in data {
            assert!(
                code.len() == 2 && code.bytes().all(|c| c.is_ascii_uppercase()),
                "{code}"
            );
            assert!(!aliases.is_empty());
            for alias in aliases {
                assert!(!alias.trim().is_empty(), "{code}");
            }
        }
    }
}
