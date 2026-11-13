//! Small fuzzy matcher for pickers, the command palette and @file search.
//! Pure functions, no dependencies. Case-insensitive. Positions are CHAR indexes (not bytes).

#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub score: i32,
    pub positions: Vec<usize>, // indexes of matched chars in the candidate, for highlighting
}

fn lower_char(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn is_boundary(prev: Option<char>, cur: char) -> bool {
    match prev {
        None => true,
        Some(p) => !p.is_alphanumeric() || (p.is_lowercase() && cur.is_uppercase()),
    }
}

fn find_sub(hay: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()] == *needle)
}

/// Match ONE term (no spaces) against a candidate.
pub fn fuzzy_match(term: &str, candidate: &str) -> Option<Match> {
    let q: Vec<char> = term.chars().map(lower_char).collect();
    if q.is_empty() {
        return Some(Match { score: 0, positions: vec![] });
    }
    let cand: Vec<char> = candidate.chars().collect();
    let lower: Vec<char> = cand.iter().map(|c| lower_char(*c)).collect();

    // 1) a contiguous substring is always the best kind of match
    if let Some(start) = find_sub(&lower, &q) {
        let mut score = 100 + 3 * q.len() as i32;
        if start == 0 {
            score += 10;
        }
        if is_boundary(start.checked_sub(1).map(|i| cand[i]), cand[start]) {
            score += 20;
        }
        score -= ((cand.len() - q.len()) as i32).min(50) / 5; // prefer shorter candidates
        return Some(Match { score, positions: (start..start + q.len()).collect() });
    }

    // 2) otherwise a subsequence, greedy from the left, with bonuses
    let mut positions = Vec::new();
    let mut score = 0;
    let mut last: Option<usize> = None;
    let mut qi = 0;
    for (i, &c) in lower.iter().enumerate() {
        if qi < q.len() && c == q[qi] {
            let mut s = 1;
            if let Some(l) = last {
                if l + 1 == i {
                    s += 10;
                } else {
                    s -= ((i - l - 1) as i32).min(5);
                }
            }
            if is_boundary(i.checked_sub(1).map(|j| cand[j]), cand[i]) {
                s += 8;
            }
            if i == 0 {
                s += 5;
            }
            score += s;
            positions.push(i);
            last = Some(i);
            qi += 1;
        }
    }
    if qi == q.len() {
        Some(Match { score, positions })
    } else {
        None
    }
}

/// Match a query that may contain several space-separated terms. ALL terms must match.
pub fn fuzzy_match_multi(query: &str, candidate: &str) -> Option<Match> {
    let mut total = 0;
    let mut positions: Vec<usize> = Vec::new();
    let mut any = false;
    for term in query.split_whitespace() {
        any = true;
        let m = fuzzy_match(term, candidate)?;
        total += m.score;
        positions.extend(m.positions);
    }
    if !any {
        return Some(Match { score: 0, positions: vec![] });
    }
    positions.sort_unstable();
    positions.dedup();
    Some(Match { score: total, positions })
}

/// Filter and rank. Returns (index into `items`, match), best first; ties keep the original order.
pub fn filter_sort<T>(query: &str, items: &[T], key: impl Fn(&T) -> String) -> Vec<(usize, Match)> {
    let mut out: Vec<(usize, Match)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, it)| fuzzy_match_multi(query, &key(it)).map(|m| (i, m)))
        .collect();
    out.sort_by(|a, b| b.1.score.cmp(&a.1.score).then(a.0.cmp(&b.0)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_matches_everything() {
        assert_eq!(fuzzy_match("", "anything").unwrap().score, 0);
        assert!(fuzzy_match_multi("   ", "x").is_some());
    }

    #[test]
    fn case_insensitive_substring_with_positions() {
        let m = fuzzy_match("FLASH", "deepseek/deepseek-v4-flash:free").unwrap();
        assert_eq!(m.positions, vec![21, 22, 23, 24, 25]);
    }

    #[test]
    fn substring_beats_subsequence() {
        let a = fuzzy_match("flash", "deepseek-v4-flash").unwrap();
        let b = fuzzy_match("flash", "f-l-a-s-h-things").unwrap();
        assert!(a.score > b.score);
    }

    #[test]
    fn subsequence_works_and_misses_return_none() {
        let m = fuzzy_match("dsf", "deepseek-flash").unwrap();
        assert_eq!(m.positions.len(), 3);
        assert!(fuzzy_match("xyz", "deepseek").is_none());
        assert!(fuzzy_match("abc", "ab").is_none());
    }

    #[test]
    fn word_start_bonus() {
        let a = fuzzy_match("cs", "claude-sonnet").unwrap(); // c at start, s after '-'
        let b = fuzzy_match("cs", "xxcxxsxx").unwrap();
        assert!(a.score > b.score);
    }

    #[test]
    fn multi_terms_all_must_match() {
        assert!(fuzzy_match_multi("or free", "openrouter/free-model").is_some());
        assert!(fuzzy_match_multi("or paid", "openrouter/free-model").is_none());
        let m = fuzzy_match_multi("deep flash", "deepseek-flash").unwrap();
        assert!(m.positions.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn filter_sort_ranks_and_is_stable() {
        let items = vec!["zeta flash", "flash", "alpha", "flashy beta", "flash"];
        let r = filter_sort("flash", &items, |s| s.to_string());
        let idx: Vec<usize> = r.iter().map(|x| x.0).collect();
        assert_eq!(idx[0], 1); // shortest exact word first
        assert_eq!(idx[1], 4); // equal score keeps original order
        assert!(!idx.contains(&2));
    }

    #[test]
    fn unicode_does_not_panic() {
        assert!(fuzzy_match("é", "Café au lait").is_some());
        assert!(fuzzy_match("日本", "日本語のモデル").is_some());
    }
}
