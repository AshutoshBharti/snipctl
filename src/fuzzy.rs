/// Built-in fuzzy search — no external dependencies needed.

/// Return a match score (lower = better) or None if no match.
///
/// Algorithm: every character in query must appear in text in order.
/// Score penalises gaps between matched characters.
pub fn fuzzy_match(query: &str, text: &str) -> Option<i32> {
    let query_lower: Vec<char> = query.to_lowercase().chars().collect();
    let text_lower: Vec<char> = text.to_lowercase().chars().collect();

    if query_lower.is_empty() {
        return Some(0);
    }

    let mut qi = 0; // query index
    let mut score: i32 = 0;
    let mut last_match: i32 = -1;

    for (ti, &ch) in text_lower.iter().enumerate() {
        if qi < query_lower.len() && ch == query_lower[qi] {
            let gap = ti as i32 - last_match - 1;
            if last_match >= 0 {
                score += gap; // penalise gaps
            }
            // bonus for matching at word boundaries
            if ti == 0 || matches!(text_lower[ti - 1], ' ' | '-' | '_' | '.' | '/' | '\\') {
                score -= 2;
            }
            last_match = ti as i32;
            qi += 1;
        }
    }

    if qi < query_lower.len() {
        return None; // not all query chars matched
    }

    // bonus: shorter text that matches fully is better
    score += text_lower.len() as i32 - query_lower.len() as i32;
    Some(score)
}

/// Filter and rank snippets by fuzzy matching against template, description, and tags.
/// Returns items sorted best-match-first.
pub fn fuzzy_filter<T, F>(query: &str, items: &[T], get_fields: F) -> Vec<usize>
where
    F: Fn(&T) -> Vec<String>,
{
    if query.trim().is_empty() {
        return (0..items.len()).collect();
    }

    let mut scored: Vec<(i32, usize)> = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        let fields = get_fields(item);
        let mut best_score: Option<i32> = None;

        for field in &fields {
            if let Some(s) = fuzzy_match(query, field) {
                if best_score.is_none() || s < best_score.unwrap() {
                    best_score = Some(s);
                }
            }
        }

        if let Some(score) = best_score {
            scored.push((score, idx));
        }
    }

    scored.sort_by_key(|&(score, _)| score);
    scored.into_iter().map(|(_, idx)| idx).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match_exact() {
        assert!(fuzzy_match("group", "az group create").is_some());
    }

    #[test]
    fn test_fuzzy_match_partial() {
        assert!(fuzzy_match("grp", "az group create").is_some());
    }

    #[test]
    fn test_fuzzy_match_no_match() {
        assert!(fuzzy_match("xyz", "az group create").is_none());
    }

    #[test]
    fn test_fuzzy_match_case_insensitive() {
        assert!(fuzzy_match("GROUP", "az group create").is_some());
    }

    #[test]
    fn test_fuzzy_filter_ranking() {
        let items = vec![
            "aws s3 cp --bucket {{bucket}}".to_string(),
            "az group create --name {{name}}".to_string(),
            "gcloud compute instances list".to_string(),
        ];

        let indices = fuzzy_filter("group", &items, |s| vec![s.clone()]);
        assert!(!indices.is_empty());
        assert_eq!(indices[0], 1); // "group" should rank highest for az group
    }

    #[test]
    fn test_fuzzy_filter_empty_query() {
        let items = vec!["a".to_string(), "b".to_string()];
        let indices = fuzzy_filter("", &items, |s| vec![s.clone()]);
        assert_eq!(indices.len(), 2);
    }
}
