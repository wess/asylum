//! Fuzzy subsequence matching and ranking.
//!
//! Powers the command palette (match action names) and quick-open (match file
//! paths). [`score`] returns `None` when `query` is not a subsequence of the
//! candidate, otherwise a higher-is-better score that rewards consecutive
//! matches, matches at word boundaries (`/`, `-`, `_`, `.`, camelCase), and an
//! early first match. [`rank`] sorts a set of candidates by score.
//!
//! Matching is case-insensitive.

/// Score `candidate` against `query`. `None` = no match.
pub fn score(query: &str, candidate: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let q: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let cand: Vec<char> = candidate.chars().collect();
    let lower: Vec<char> = candidate.chars().flat_map(char::to_lowercase).collect();

    let mut qi = 0usize;
    let mut total = 0i32;
    let mut prev_match: Option<usize> = None;

    for (ci, &lc) in lower.iter().enumerate() {
        if qi >= q.len() {
            break;
        }
        if lc == q[qi] {
            let mut points = 1;
            // Consecutive match bonus — the dominant signal (fzf-style): a run
            // of adjacent matches beats the same characters scattered across
            // word boundaries.
            if prev_match == Some(ci.wrapping_sub(1)) {
                points += 10;
            }
            // Word-boundary bonus.
            if ci == 0 || is_boundary(&cand, ci) {
                points += 6;
            }
            // Early-match bonus (first matched char).
            if qi == 0 {
                points += (10i32 - ci as i32).max(0);
            }
            total += points;
            prev_match = Some(ci);
            qi += 1;
        }
    }

    if qi == q.len() {
        // Prefer shorter candidates on ties.
        Some(total - (cand.len() as i32) / 20)
    } else {
        None
    }
}

/// True when the character at `i` starts a new "word" (previous char is a
/// separator, or this is a camelCase hump).
fn is_boundary(cand: &[char], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let prev = cand[i - 1];
    if matches!(prev, '/' | '\\' | '-' | '_' | '.' | ' ' | ':') {
        return true;
    }
    // camelCase: lower→Upper transition.
    prev.is_lowercase() && cand[i].is_uppercase()
}

/// A ranked match: the candidate index, the candidate, and its score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match<'a> {
    pub index: usize,
    pub value: &'a str,
    pub score: i32,
}

/// Rank `candidates` against `query`, best first. Non-matches are dropped. With
/// an empty query, returns all candidates in their original order.
pub fn rank<'a, I, S>(query: &str, candidates: I) -> Vec<Match<'a>>
where
    I: IntoIterator<Item = &'a S>,
    S: AsRef<str> + 'a + ?Sized,
{
    let mut matches: Vec<Match<'a>> = candidates
        .into_iter()
        .enumerate()
        .filter_map(|(index, c)| {
            let value = c.as_ref();
            score(query, value).map(|score| Match { index, value, score })
        })
        .collect();
    // Stable sort by score desc; equal scores keep input order.
    matches.sort_by_key(|m| std::cmp::Reverse(m.score));
    matches
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
