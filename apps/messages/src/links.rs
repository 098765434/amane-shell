//! What a message holds that can be used: its links, a one-time code to
//! copy, and whether it's only a few emoji, shown large without a bubble.

/// The links in `text`: `http://`, `https://` and `www.` ones, without
/// the punctuation that ends a sentence after them.
pub fn links(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for word in text.split_whitespace() {
        let word = word.trim_start_matches(['(', '<', '"', '\'', '[']);
        let lower = word.to_ascii_lowercase();
        let starts = lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("www.");
        if !starts {
            continue;
        }
        let link = word.trim_end_matches(['.', ',', '!', '?', ')', '>', '"', '\'', ']', ';', ':']);
        // "www." alone, or a scheme with nothing after it, is no link
        let rest = link.split_once("//").map_or(link, |(_, rest)| rest);
        if rest.len() > 4 && rest.contains('.') {
            found.push(link.to_string());
        }
    }
    found
}

/// A link as it opens: `www.` ones get `https://`.
pub fn url(link: &str) -> String {
    if link.to_ascii_lowercase().starts_with("www.") {
        format!("https://{link}")
    } else {
        link.to_string()
    }
}

/// A link as it's shown: without its scheme or a trailing slash.
pub fn shown(link: &str) -> String {
    let bare = link
        .split_once("://")
        .map_or(link, |(_, rest)| rest)
        .trim_end_matches('/');
    bare.strip_prefix("www.").unwrap_or(bare).to_string()
}

/// A one-time code in `text`: a run of 4 to 8 digits standing on its own
/// (a dash or space may split it in two, "123-456"), in a message that
/// talks about a code. Numbers in other messages (prices, dates, phone
/// numbers) aren't codes.
pub fn code(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    const WORDS: [&str; 10] = [
        "code",
        "kode",
        "otp",
        "pin",
        "passcode",
        "verification",
        "verify",
        "engangs",
        "password",
        "passord",
    ];
    if !WORDS.iter().any(|word| lower.contains(word)) {
        return None;
    }

    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if !chars[index].is_ascii_digit() || (index > 0 && is_part(chars[index - 1])) {
            index += 1;
            continue;
        }
        // the digits, with one dash or space between two groups of three
        let mut digits = String::new();
        let mut end = index;
        while end < chars.len() {
            let c = chars[end];
            if c.is_ascii_digit() {
                digits.push(c);
                end += 1;
            } else if (c == '-' || c == ' ')
                && digits.len() == 3
                && chars.get(end + 1).is_some_and(char::is_ascii_digit)
            {
                end += 1;
            } else {
                break;
            }
        }
        // a full stop or comma after it ends the sentence, unless a decimal follows
        let alone = match chars.get(end) {
            None => true,
            Some('.' | ',' | ':') => !chars.get(end + 1).is_some_and(char::is_ascii_digit),
            Some(next) => !is_part(*next),
        };
        if alone && (4..=8).contains(&digits.len()) {
            return Some(digits);
        }
        index = end.max(index + 1);
    }
    None
}

/// A character that makes digits next to it part of something else: more
/// digits, a word, a decimal point or a phone number's plus.
fn is_part(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '+' | '/' | '.' | ',' | ':' | '€' | '$' | '£')
}

/// Whether `text` is one to three emoji and nothing else. An emoji
/// joined to the one before (a family) or a flag's second letter isn't
/// another one.
pub fn emoji_only(text: &str) -> bool {
    let mut count = 0;
    let mut joined = false;
    let mut flag_half = false;
    for c in text.trim().chars() {
        match c {
            '\u{200d}' => joined = true,
            // what varies emoji, and the skin tones
            '\u{fe0f}' | '\u{fe0e}' | '\u{20e3}' | '\u{1f3fb}'..='\u{1f3ff}' => {}
            c if c.is_whitespace() => {}
            '\u{1f1e6}'..='\u{1f1ff}' => {
                if !flag_half {
                    count += 1;
                }
                flag_half = !flag_half;
            }
            c if is_emoji(c) => {
                if !joined {
                    count += 1;
                }
                joined = false;
            }
            _ => return false,
        }
    }
    (1..=3).contains(&count)
}

fn is_emoji(c: char) -> bool {
    matches!(c,
        '\u{1f300}'..='\u{1faff}'
        | '\u{2600}'..='\u{27bf}'
        | '\u{1f000}'..='\u{1f2ff}'
        | '\u{2b00}'..='\u{2bff}'
        | '\u{2300}'..='\u{23ff}'
        | '\u{2190}'..='\u{21ff}'
        | '\u{3030}' | '\u{303d}' | '\u{3297}' | '\u{3299}' | '\u{a9}' | '\u{ae}' | '\u{2122}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_links_without_the_sentence_around_them() {
        assert_eq!(
            links("Se https://example.com/a?b=1. Og (www.nrk.no)!"),
            ["https://example.com/a?b=1", "www.nrk.no"]
        );
        assert!(links("www. og http:// er ingenting").is_empty());
        assert_eq!(url("www.nrk.no"), "https://www.nrk.no");
        assert_eq!(shown("https://www.nrk.no/"), "nrk.no");
    }

    #[test]
    fn finds_a_code_only_where_one_is_meant() {
        assert_eq!(code("Your code is 482913").as_deref(), Some("482913"));
        assert_eq!(code("Din kode: 123-456").as_deref(), Some("123456"));
        assert_eq!(
            code("G-58210 is your Google verification code").as_deref(),
            Some("58210")
        );
        assert_eq!(
            code("Verification code 5821, valid 10 min").as_deref(),
            Some("5821")
        );
        assert_eq!(code("Ring meg på 95723015"), None);
        assert_eq!(code("Code: call +4795723015"), None);
        assert_eq!(code("Total 1299.00 kr, code ABC"), None);
    }

    #[test]
    fn a_few_emoji_alone_are_shown_large() {
        assert!(emoji_only("😂"));
        assert!(emoji_only(" ❤️ 👍 "));
        assert!(emoji_only("👍🏽"));
        assert!(emoji_only("👨‍👩‍👧‍👦🇳🇴"));
        assert!(!emoji_only("😂 haha"));
        assert!(!emoji_only("😂😂😂😂"));
        assert!(!emoji_only(""));
        assert!(!emoji_only("123"));
    }
}
