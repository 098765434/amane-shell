//! How well a few typed letters match a name: the launcher's results and
//! Emaki Settings' search both rank by it.

/// How well `needle` (already lowercased) matches `hay`, 0 … 1, or `None`
/// when it doesn't: the whole name, then its start, a word's start, the
/// words' initials ("vsc" for Visual Studio Code), anywhere in it, and last
/// its letters in order with others between.
pub fn fuzzy(needle: &str, hay: &str) -> Option<f32> {
    if needle.is_empty() {
        return Some(0.0);
    }
    let hay = hay.to_lowercase();
    // a longer name matched by the same letters is a little less likely
    let share = needle.chars().count() as f32 / hay.chars().count().max(1) as f32;
    if hay == needle {
        return Some(1.0);
    }
    if hay.starts_with(needle) {
        return Some(0.85 + 0.1 * share);
    }
    let words: Vec<&str> = hay.split(is_separator).filter(|w| !w.is_empty()).collect();
    if words.iter().any(|word| word.starts_with(needle)) {
        return Some(0.75 + 0.1 * share);
    }
    let initials: String = words
        .iter()
        .filter_map(|word| word.chars().next())
        .collect();
    if needle.chars().count() >= 2 && initials.starts_with(needle) {
        return Some(0.7);
    }
    if hay.contains(needle) {
        return Some(0.55 + 0.1 * share);
    }
    subsequence(needle, &hay)
}

fn is_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, '-' | '_' | '.' | '/' | ':' | '(' | ')')
}

/// `needle`'s letters in order through `hay`, scored by how many come
/// together; fewer than three letters scattered through it aren't a match
/// (two would match most names).
fn subsequence(needle: &str, hay: &str) -> Option<f32> {
    let wanted: Vec<char> = needle.chars().filter(|c| !c.is_whitespace()).collect();
    if wanted.len() < 3 {
        return None;
    }
    let mut at = 0;
    let mut runs = 0;
    let mut last: Option<usize> = None;
    for (index, c) in hay.chars().enumerate() {
        if at < wanted.len() && c == wanted[at] {
            if last.is_some_and(|last| last + 1 == index) {
                runs += 1;
            }
            last = Some(index);
            at += 1;
        }
    }
    (at == wanted.len()).then(|| 0.3 + 0.2 * runs as f32 / (wanted.len() - 1) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(needle: &str, hay: &str) -> f32 {
        fuzzy(needle, hay).unwrap_or(-1.0)
    }

    #[test]
    fn a_closer_match_scores_higher() {
        assert_eq!(score("firefox", "Firefox"), 1.0);
        assert!(score("fire", "Firefox") > score("fox", "Firefox"));
        assert!(score("code", "Visual Studio Code") > score("od", "Visual Studio Code"));
        assert!(score("vsc", "Visual Studio Code") > score("vsd", "Visual Studio Code"));
    }

    #[test]
    fn letters_in_order_still_match() {
        assert!(fuzzy("frx", "firefox").is_some());
        assert!(fuzzy("xf", "firefox").is_none());
        // two letters scattered through a name are too many names
        assert!(fuzzy("tm", "bluetooth manager").is_none());
        // one letter has to be in the name itself
        assert!(fuzzy("z", "firefox").is_none());
        assert!(fuzzy("f", "firefox").is_some());
    }

    #[test]
    fn letters_together_beat_letters_apart() {
        assert!(score("tex", "kate editor text") > score("tdr", "kate editor text"));
    }
}
