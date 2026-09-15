//! Fuzzy search, ported verbatim from Obsidian's `FuzzyScore` / `NormalizeSearch`.

/// Score a (lowercase) `text` against a normalized `search`.
/// Returns `None` when it does not match, `Some(score)` otherwise (higher is better).
pub fn fuzzy_score(text: &str, search: &str) -> Option<f32> {
    if search.is_empty() {
        return Some(0.0);
    }
    if text.is_empty() {
        return None;
    }
    // Fast path: literal substring (also the best possible score).
    if let Some(idx) = text.find(search) {
        let at_boundary = idx == 0 || is_boundary(text[..idx].chars().next_back());
        let exact_idx = text[..idx].chars().count() as f32 + 1.0;
        return Some(1e5 - exact_idx + if at_boundary { 500.0 } else { 0.0 } + search.chars().count() as f32 * 5.0);
    }
    let tchars: Vec<char> = text.chars().collect();
    let schars: Vec<char> = search.chars().collect();
    if schars.len() > tchars.len() {
        return None;
    }
    let mut si = 0usize;
    let mut score = 0.0f32;
    let mut run = 0i32;
    let mut last_match: i32 = 0; // 1-based index of the last matched char
    for (i0, &tc) in tchars.iter().enumerate() {
        if si >= schars.len() {
            break;
        }
        let ti = i0 as i32 + 1;
        if tc == schars[si] {
            let at_boundary = ti == 1 || is_boundary(Some(tchars[i0 - 1]));
            run = if last_match == ti - 1 { run + 1 } else { 1 };
            score += 1.0 + if at_boundary { 6.0 } else { 0.0 } + (run - 1).min(5) as f32 * 3.0;
            last_match = ti;
            si += 1;
        }
    }
    if si < schars.len() {
        return None;
    }
    score -= (last_match - schars.len() as i32) as f32 * 0.05;
    Some(score)
}

fn is_boundary(prev: Option<char>) -> bool {
    match prev {
        None => true,
        Some(c) => c.is_whitespace() || c.is_ascii_punctuation() || c == '_',
    }
}

/// Lowercase and strip all whitespace.
pub fn normalize(search: &str) -> String {
    search.chars().filter(|c| !c.is_whitespace()).flat_map(|c| c.to_lowercase()).collect()
}

/// False for empty text; lowercases the text.
pub fn matches(text: &str, search: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let lower = text.to_lowercase();
    fuzzy_score(&lower, search).is_some()
}

/// Maximum number of dropdown values inspected by [`matches_values`].
pub const VALUE_CAP: usize = 200;

/// `MatchesSearch` value scan: any of the first 200 values (display or raw) matches.
pub fn matches_values<'a>(values: impl IntoIterator<Item = (&'a str, &'a str)>, search: &str) -> bool {
    for (i, (value, display)) in values.into_iter().enumerate() {
        if i >= VALUE_CAP {
            break;
        }
        if matches(display, search) || matches(value, search) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substring_beats_fuzzy_and_boundary_bonus_applies() {
        let sub = fuzzy_score("aimbot settings", "set").unwrap();
        let fuzzy = fuzzy_score("silent aim", "set").unwrap();
        assert!(sub > fuzzy);
        let boundary = fuzzy_score("aim bot", "bot").unwrap();
        let inner = fuzzy_score("aimbot", "bot").unwrap();
        assert!(boundary > inner);
        assert_eq!(fuzzy_score("abc", "abcd"), None);
        assert_eq!(fuzzy_score("xyz", "a"), None);
        assert_eq!(fuzzy_score("", "a"), None);
        assert_eq!(fuzzy_score("anything", ""), Some(0.0));
    }

    #[test]
    fn fuzzy_requires_in_order_characters() {
        assert!(fuzzy_score("configuration", "cfg").is_some());
        assert!(fuzzy_score("configuration", "gfc").is_none());
    }

    #[test]
    fn normalize_strips_whitespace_and_case() {
        assert_eq!(normalize("  Aim Bot "), "aimbot");
        assert!(matches("Aim Bot", "aimbot"));
        assert!(!matches("", "a"));
    }

    #[test]
    fn value_scan_is_capped() {
        let values: Vec<(String, String)> = (0..300).map(|i| (format!("v{i}"), format!("v{i}"))).collect();
        let iter = || values.iter().map(|(a, b)| (a.as_str(), b.as_str()));
        assert!(matches_values(iter(), "v199"));
        assert!(!matches_values(iter(), "v250"));
    }
}
