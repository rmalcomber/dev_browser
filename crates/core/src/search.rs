//! Fuzzy matching for the launcher. Pure text in, a score out: no data, no scope (scope is decided in `backend`, before anything is matched).

/// How well `query` matches `text`, or `None` if it does not. Every whitespace-separated word of the query must match, in order of
/// letters (a subsequence, case ignored). Higher is better. An empty query matches everything with score 0.
pub fn score(query: &str, text: &str) -> Option<u32> {
    let hay: Vec<char> = text.to_lowercase().chars().collect();
    let mut total = 0;
    for word in query.split_whitespace() {
        total += score_word(&word.to_lowercase().chars().collect::<Vec<_>>(), &hay)?;
    }
    // a shorter text that matches is a better match than a long one
    Some(total.saturating_sub((hay.len() / 8) as u32))
}

fn score_word(word: &[char], hay: &[char]) -> Option<u32> {
    if word.is_empty() {
        return Some(0);
    }
    // a contiguous match is the best kind; at the start of the text or of a word it is better still
    if let Some(at) = find(hay, word) {
        let mut s = 100 + 10 * word.len() as u32;
        if at == 0 {
            s += 40;
        } else if !hay[at - 1].is_alphanumeric() {
            s += 25;
        }
        return Some(s);
    }
    // otherwise a subsequence, rewarding consecutive letters and letters at the start of a word
    let mut s = 0;
    let (mut i, mut prev) = (0, None::<usize>);
    for &c in word {
        let at = (i..hay.len()).find(|&k| hay[k] == c)?;
        s += 10;
        if prev.is_some_and(|p| p + 1 == at) {
            s += 15;
        }
        if at == 0 || !hay[at - 1].is_alphanumeric() {
            s += 20;
        }
        prev = Some(at);
        i = at + 1;
    }
    Some(s)
}

/// A small nudge for results the user opens often and recently: up to about 19 for frequency and 8 for recency. It never decides what
/// matches, only the order of what already does, and only within the scope that was already allowed.
pub fn usage_boost(count: i64, age_secs: i64) -> u32 {
    let frequency = ((count.max(0) as f64 + 1.0).ln() * 8.0).min(19.0);
    let recency = match age_secs {
        a if a < 0 => 0.0,
        a if a < 3600 => 8.0,
        a if a < 86_400 => 4.0,
        a if a < 7 * 86_400 => 2.0,
        _ => 0.0,
    };
    (frequency + recency) as u32
}

fn find(hay: &[char], word: &[char]) -> Option<usize> {
    if word.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - word.len()).find(|&i| hay[i..i + word.len()] == *word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_match_without_the_letters_in_order() {
        assert_eq!(score("xyz", "github"), None);
        assert_eq!(score("hg", "github"), None, "letters out of order");
    }

    #[test]
    fn empty_query_matches_everything() {
        assert_eq!(score("", "anything"), Some(0));
        assert_eq!(score("   ", "anything"), Some(0));
    }

    #[test]
    fn case_is_ignored_and_every_word_must_match() {
        assert!(score("GIT hub", "GitHub - home").is_some());
        assert_eq!(score("git zebra", "GitHub - home"), None);
    }

    #[test]
    fn better_matches_score_higher() {
        let prefix = score("git", "GitHub").unwrap();
        let word = score("git", "My GitHub").unwrap();
        let inside = score("git", "Digit").unwrap();
        let scattered = score("gtb", "GitHub").unwrap();
        assert!(
            prefix > word && word > inside && inside > scattered,
            "{prefix} {word} {inside} {scattered}"
        );
    }

    #[test]
    fn shorter_text_wins_a_tie() {
        assert!(
            score("new", "New tab").unwrap()
                > score("new", "New tab in a workspace that has a very long name").unwrap()
        );
    }

    #[test]
    fn the_usage_boost_is_small_and_grows_with_use_and_recency() {
        assert_eq!(usage_boost(0, 0), 8);
        assert!(usage_boost(5, 10) > usage_boost(1, 10));
        assert!(usage_boost(5, 10) > usage_boost(5, 10 * 86_400));
        assert!(usage_boost(1_000_000, 0) <= 27, "bounded");
        assert_eq!(usage_boost(-5, -5), 0);
        assert!(
            usage_boost(3, 100) < 100,
            "never as big as a contiguous-match bonus"
        );
    }

    #[test]
    fn odd_text_does_not_panic() {
        for t in ["", "é", "日本語", "\u{0}", "a\nb"] {
            let _ = score("a", t);
            let _ = score("é", t);
        }
    }
}
