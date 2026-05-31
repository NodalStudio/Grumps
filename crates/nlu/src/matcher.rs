use strsim::jaro_winkler;

#[derive(Debug, Clone, PartialEq)]
pub enum MatchResult {
    Exact(MatchedTodo),
    Fuzzy(Vec<MatchedTodo>),
    NoMatch,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchedTodo {
    pub todo_id: String,
    pub title: String,
    pub seq_num: i64,
    pub score: f64,
}

/// Match user input against a list of open todos.
/// Each entry is `(todo_id, title, seq_num)`.
/// Returns Exact when score > 0.9, Fuzzy (top 3) when score 0.4–0.9, NoMatch otherwise.
pub fn match_done(input: &str, open_todos: &[(String, String, i64)]) -> MatchResult {
    let norm = normalize(input);
    let mut scored: Vec<MatchedTodo> = open_todos
        .iter()
        .map(|(id, title, seq)| {
            let nt = normalize(title);
            let jw = jaro_winkler(&norm, &nt);
            let tok = token_overlap(&norm, &nt);
            let score = jw * 0.6 + tok * 0.4;
            MatchedTodo {
                todo_id: id.clone(),
                title: title.clone(),
                seq_num: *seq,
                score,
            }
        })
        .filter(|m| m.score > 0.4)
        .collect();

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    match scored.first() {
        Some(best) if best.score > 0.9 => MatchResult::Exact(best.clone()),
        Some(_) => {
            scored.truncate(3);
            MatchResult::Fuzzy(scored)
        }
        None => MatchResult::NoMatch,
    }
}

fn normalize(text: &str) -> String {
    let cleaned: String = text
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();

    const STOPWORDS: &[&str] = &[
        "the", "a", "an", "is", "to", "for", "of", "in", "on", "at", "le", "la", "les", "un",
        "une", "de", "du", "des", "et", "est",
    ];

    cleaned
        .split_whitespace()
        .filter(|w| !STOPWORDS.contains(w))
        .collect::<Vec<_>>()
        .join(" ")
}

fn token_overlap(a: &str, b: &str) -> f64 {
    let at: Vec<&str> = a.split_whitespace().collect();
    let bt: Vec<&str> = b.split_whitespace().collect();
    if at.is_empty() {
        return 0.0;
    }
    at.iter().filter(|t| bt.contains(t)).count() as f64 / at.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn todos(list: &[(&str, &str, i64)]) -> Vec<(String, String, i64)> {
        list.iter()
            .map(|(id, title, seq)| (id.to_string(), title.to_string(), *seq))
            .collect()
    }

    #[test]
    fn exact_match_buy_toilet_paper() {
        let db = todos(&[
            ("id1", "Buy toilet paper", 1),
            ("id2", "Call the plumber", 2),
            ("id3", "Pay electricity bill", 3),
        ]);
        let result = match_done("buy toilet paper", &db);
        assert!(
            matches!(result, MatchResult::Exact(ref m) if m.todo_id == "id1"),
            "Expected Exact match for 'buy toilet paper', got {:?}",
            result
        );
    }

    #[test]
    fn exact_match_case_insensitive() {
        let db = todos(&[("id1", "Buy Toilet Paper", 1)]);
        let result = match_done("BUY TOILET PAPER", &db);
        assert!(matches!(result, MatchResult::Exact(_)), "{:?}", result);
    }

    #[test]
    fn partial_input_high_score() {
        // "toilet paper" should still score highly against "Buy toilet paper"
        let db = todos(&[("id1", "Buy toilet paper", 1), ("id2", "Unrelated task", 2)]);
        let result = match_done("toilet paper", &db);
        // Should be Exact or at least the top fuzzy match is id1
        match &result {
            MatchResult::Exact(m) => assert_eq!(m.todo_id, "id1"),
            MatchResult::Fuzzy(matches) => assert_eq!(matches[0].todo_id, "id1"),
            MatchResult::NoMatch => panic!("Expected a match, got NoMatch"),
        }
    }

    #[test]
    fn fuzzy_multiple_matches() {
        let db = todos(&[
            ("id1", "Buy toilet paper", 1),
            ("id2", "Call the plumber about the leak", 2),
            ("id3", "Pay plumber invoice", 3),
            ("id4", "Unrelated task xyz", 4),
        ]);
        // "plumber" should match both plumber todos
        let result = match_done("plumber", &db);
        match result {
            MatchResult::Fuzzy(matches) => {
                let ids: Vec<&str> = matches.iter().map(|m| m.todo_id.as_str()).collect();
                assert!(
                    ids.contains(&"id2") || ids.contains(&"id3"),
                    "Expected plumber matches, got {:?}",
                    ids
                );
                assert!(matches.len() <= 3);
            }
            MatchResult::Exact(m) => {
                // Acceptable if one is a near-perfect match
                assert!(m.todo_id == "id2" || m.todo_id == "id3");
            }
            MatchResult::NoMatch => panic!("Expected fuzzy match for 'plumber'"),
        }
    }

    #[test]
    fn no_match_unrelated() {
        let db = todos(&[
            ("id1", "Buy toilet paper", 1),
            ("id2", "Call the plumber", 2),
        ]);
        let result = match_done("xyz unrelated qwerty", &db);
        assert_eq!(result, MatchResult::NoMatch, "{:?}", result);
    }

    #[test]
    fn no_match_empty_list() {
        let result = match_done("buy toilet paper", &[]);
        assert_eq!(result, MatchResult::NoMatch);
    }

    #[test]
    fn fuzzy_truncated_to_three() {
        let db = todos(&[
            ("id1", "Fix the bug in module", 1),
            ("id2", "Fix the error in module", 2),
            ("id3", "Fix the issue in module", 3),
            ("id4", "Fix the problem in module", 4),
            ("id5", "Fix the flaw in module", 5),
        ]);
        let result = match_done("fix module", &db);
        match result {
            MatchResult::Fuzzy(matches) => assert!(matches.len() <= 3),
            MatchResult::Exact(_) => {} // also acceptable
            MatchResult::NoMatch => panic!("Expected some match"),
        }
    }

    #[test]
    fn normalize_strips_stopwords() {
        // "the", "a", "of" should be stripped
        let n = super::normalize("the call of duty");
        assert!(!n.contains("the"), "stopword 'the' not removed: '{n}'");
        assert!(!n.contains(" of "), "stopword 'of' not removed: '{n}'");
        assert!(n.contains("call"), "content word 'call' missing: '{n}'");
        assert!(n.contains("duty"), "content word 'duty' missing: '{n}'");
    }

    #[test]
    fn normalize_strips_punctuation() {
        let n = super::normalize("buy milk, eggs!");
        assert!(!n.contains(','), "comma not removed: '{n}'");
        assert!(!n.contains('!'), "exclamation not removed: '{n}'");
    }

    #[test]
    fn normalize_lowercases() {
        let n = super::normalize("Buy MILK");
        assert_eq!(n, "buy milk");
    }

    #[test]
    fn token_overlap_all_match() {
        let score = super::token_overlap("buy milk", "buy milk today");
        assert!((score - 1.0).abs() < f64::EPSILON, "score={score}");
    }

    #[test]
    fn token_overlap_none_match() {
        let score = super::token_overlap("buy milk", "call plumber");
        assert!(score < f64::EPSILON, "score={score}");
    }

    #[test]
    fn token_overlap_partial() {
        // "buy milk eggs" vs "buy bread eggs" → 2/3 match
        let score = super::token_overlap("buy milk eggs", "buy bread eggs");
        assert!((score - 2.0 / 3.0).abs() < 1e-9, "score={score}");
    }

    #[test]
    fn token_overlap_empty_a() {
        let score = super::token_overlap("", "buy milk");
        assert!(score < f64::EPSILON, "score={score}");
    }

    #[test]
    fn fuzzy_results_sorted_by_score_descending() {
        let db = todos(&[
            ("id1", "Call the plumber about the leak in the basement", 1),
            ("id2", "Pay plumber invoice", 2),
        ]);
        let result = match_done("plumber", &db);
        match result {
            MatchResult::Fuzzy(matches) => {
                for w in matches.windows(2) {
                    assert!(
                        w[0].score >= w[1].score,
                        "Results not sorted: {:?}",
                        matches
                    );
                }
            }
            MatchResult::Exact(_) => {} // fine
            MatchResult::NoMatch => panic!("Expected a match"),
        }
    }

    #[test]
    fn french_stopwords_stripped() {
        let n = super::normalize("le chat est sur le tapis");
        assert!(!n.contains(" le "), "french stopword 'le' not removed");
        assert!(!n.contains("est"), "french stopword 'est' not removed");
        assert!(n.contains("chat"), "'chat' missing: '{n}'");
        assert!(n.contains("tapis"), "'tapis' missing: '{n}'");
    }
}
