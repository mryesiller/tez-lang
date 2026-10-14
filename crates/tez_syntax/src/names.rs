//! Naming convention checks and fixes: values `lowerCamel`, types `UpperCamel`.

/// True for `lowerCamel` (starts lowercase, ASCII alphanumerics only).
#[must_use]
pub fn is_lower_camel(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_lowercase()) && s.chars().all(|c| c.is_ascii_alphanumeric())
}

/// True for `UpperCamel`.
#[must_use]
pub fn is_upper_camel(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_uppercase()) && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn words(s: &str) -> Vec<String> {
    let parts: Vec<&str> = s.split('_').filter(|p| !p.is_empty()).collect();
    parts
        .iter()
        .map(|p| {
            let all_caps = p
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
            if all_caps {
                p.to_ascii_lowercase()
            } else {
                (*p).to_string()
            }
        })
        .collect()
}

fn capitalize(s: &str) -> String {
    let mut cs = s.chars();
    cs.next().map_or_else(String::new, |f| {
        f.to_ascii_uppercase().to_string() + cs.as_str()
    })
}

fn lowercase_first(s: &str) -> String {
    let mut cs = s.chars();
    cs.next().map_or_else(String::new, |f| {
        f.to_ascii_lowercase().to_string() + cs.as_str()
    })
}

/// Converts `snake_case`, `SCREAMING_CASE` or `Pascal` to `lowerCamel`.
#[must_use]
pub fn to_lower_camel(s: &str) -> String {
    let ws = words(s);
    let mut out = String::new();
    for (i, w) in ws.iter().enumerate() {
        if i == 0 {
            out.push_str(&lowercase_first(w));
        } else {
            out.push_str(&capitalize(w));
        }
    }
    if out.is_empty() { "name".into() } else { out }
}

/// Converts to `UpperCamel`.
#[must_use]
pub fn to_upper_camel(s: &str) -> String {
    let ws = words(s);
    let out: String = ws.iter().map(|w| capitalize(w)).collect();
    if out.is_empty() { "Name".into() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification() {
        assert!(is_lower_camel("fooBar1"));
        assert!(!is_lower_camel("foo_bar"));
        assert!(!is_lower_camel("Foo"));
        assert!(!is_lower_camel("_x"));
        assert!(is_upper_camel("FooBar"));
        assert!(!is_upper_camel("FOO_BAR"));
    }

    #[test]
    fn conversions() {
        assert_eq!(to_lower_camel("max_size"), "maxSize");
        assert_eq!(to_lower_camel("MAX_SIZE"), "maxSize");
        assert_eq!(to_lower_camel("Point"), "point");
        assert_eq!(to_upper_camel("shape_kind"), "ShapeKind");
        assert_eq!(to_upper_camel("point"), "Point");
        assert_eq!(to_lower_camel("_x"), "x");
    }
}
