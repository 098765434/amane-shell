//! What the launcher works out without a compositor: which pill an app
//! goes under, what a query is (a sum, a URL, a session action's name), how
//! a URL is spelled, where Up and Down go through the searches run
//! before, how long a duration typed is, and what quick keys can be. The
//! view reads it, and the tests run it.

use std::time::Duration;

/// The pills, in the order they show. A bucket's index is its bit in an
/// app's mask; "All Applications" holds every app.
pub(super) const CATEGORIES: [&str; 11] = [
    "All Applications",
    "Chrome Apps",
    "Development",
    "Education",
    "Games",
    "Graphics",
    "Help",
    "Internet",
    "Multimedia",
    "Office",
    "Utilities",
];

/// Where a web search goes, the query appended.
pub(super) const SEARCH_URL: &str = "https://www.google.com/search?q=";

fn bit(bucket: usize) -> u16 {
    1 << bucket
}

/// The buckets an app falls into, from its freedesktop `Categories` and its
/// command line. Chrome Apps and Help have no category of their own, so
/// they're guessed: a Chromium web app's launcher passes `--app=`, and a
/// help viewer has a category with "Help" in it.
pub(super) fn mask(categories: &[String], exec: &str) -> u16 {
    let mut mask = 0;
    for category in categories {
        match category.as_str() {
            "Development" => mask |= bit(2),
            "Education" => mask |= bit(3),
            "Game" => mask |= bit(4),
            "Graphics" => mask |= bit(5),
            "Network" => mask |= bit(7),
            "AudioVideo" | "Audio" | "Video" => mask |= bit(8),
            "Office" => mask |= bit(9),
            "Utility" | "System" | "Settings" => mask |= bit(10),
            _ => {}
        }
        if category.contains("Help") {
            mask |= bit(6);
        }
    }
    if exec.contains("--app=") {
        mask |= bit(1);
    }
    mask
}

/// Whether an app with `mask` shows under pill `bucket`.
pub(super) fn in_bucket(mask: u16, bucket: usize) -> bool {
    bucket == 0 || mask & bit(bucket) != 0
}

/// "All Applications" and every pill at least one app falls under, in
/// order: a pill with nothing under it isn't shown.
pub(super) fn active_buckets(masks: impl IntoIterator<Item = u16>) -> Vec<usize> {
    let present = masks.into_iter().fold(0, |all, mask| all | mask);
    (0..CATEGORIES.len())
        .filter(|&bucket| bucket == 0 || present & bit(bucket) != 0)
        .collect()
}

/// Where Up (`older`) or Down goes from `at` through `len` searches run
/// before, newest first, `None` being the empty field; `None` when there's
/// nowhere further to go.
pub(super) fn history_step(at: Option<usize>, older: bool, len: usize) -> Option<Option<usize>> {
    match (at, older) {
        (None, true) if len > 0 => Some(Some(0)),
        (Some(at), true) if at + 1 < len => Some(Some(at + 1)),
        (Some(0), false) => Some(None),
        (Some(at), false) => Some(Some(at - 1)),
        _ => None,
    }
}

/// A session action the search offers by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SessionAction {
    Lock,
    LogOut,
    Suspend,
    Restart,
    PowerOff,
}

impl SessionAction {
    pub(super) const ALL: [Self; 5] = [
        Self::Lock,
        Self::LogOut,
        Self::Suspend,
        Self::Restart,
        Self::PowerOff,
    ];

    /// What its row says: the power menu's words.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Lock => "Lock Screen",
            Self::LogOut => "Log Out",
            Self::Suspend => "Suspend",
            Self::Restart => "Restart",
            Self::PowerOff => "Power Off",
        }
    }

    /// What its action is kept under.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Lock => "lock",
            Self::LogOut => "log-out",
            Self::Suspend => "suspend",
            Self::Restart => "restart",
            Self::PowerOff => "power-off",
        }
    }

    /// The other words it's found by.
    pub(super) fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Lock => &["lock"],
            Self::LogOut => &["logout", "sign out", "quit"],
            Self::Suspend => &["sleep"],
            Self::Restart => &["reboot"],
            Self::PowerOff => &["shut down", "shutdown", "poweroff", "turn off"],
        }
    }
}

/// `query` as a sum's answer: `+ - * / ( )` over decimal numbers. `None`
/// unless all of it parses and it has an operator, so "42" and "firefox"
/// aren't sums.
pub(super) fn evaluate(query: &str) -> Option<f64> {
    if !query.contains(['+', '-', '*', '/', '(', ')']) {
        return None;
    }
    let tokens: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).collect();
    let mut parser = Parser {
        tokens: &tokens,
        at: 0,
    };
    let value = parser.sum()?;
    (parser.at == tokens.len() && value.is_finite()).then_some(value)
}

struct Parser<'a> {
    tokens: &'a [char],
    at: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.tokens.get(self.at).copied()
    }

    // sum := product (('+' | '-') product)*
    fn sum(&mut self) -> Option<f64> {
        let mut value = self.product()?;
        while let Some(op @ ('+' | '-')) = self.peek() {
            self.at += 1;
            let rhs = self.product()?;
            value = if op == '+' { value + rhs } else { value - rhs };
        }
        Some(value)
    }

    // product := factor (('*' | '/') factor)*
    fn product(&mut self) -> Option<f64> {
        let mut value = self.factor()?;
        while let Some(op @ ('*' | '/')) = self.peek() {
            self.at += 1;
            let rhs = self.factor()?;
            value = if op == '*' { value * rhs } else { value / rhs };
        }
        Some(value)
    }

    // factor := ('-' | '+') factor | '(' sum ')' | number
    fn factor(&mut self) -> Option<f64> {
        match self.peek() {
            Some('-') => {
                self.at += 1;
                Some(-self.factor()?)
            }
            Some('+') => {
                self.at += 1;
                self.factor()
            }
            Some('(') => {
                self.at += 1;
                let value = self.sum()?;
                (self.peek() == Some(')')).then(|| {
                    self.at += 1;
                    value
                })
            }
            _ => self.number(),
        }
    }

    fn number(&mut self) -> Option<f64> {
        let start = self.at;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit() || c == '.') {
            self.at += 1;
        }
        if self.at == start {
            return None;
        }
        let text: String = self.tokens[start..self.at].iter().collect();
        text.parse().ok()
    }
}

/// A sum's answer as it's shown and copied: no trailing zeros.
pub(super) fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        let text = format!("{value:.6}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Whether `query` reads as a URL: no spaces, and a scheme or a host with
/// a top-level domain.
pub(super) fn looks_like_url(query: &str) -> bool {
    if query.is_empty() || query.chars().any(char::is_whitespace) {
        return false;
    }
    if query.starts_with("http://") || query.starts_with("https://") {
        return true;
    }
    let host = query.split('/').next().unwrap_or(query);
    match host.rsplit_once('.') {
        Some((name, tld)) => {
            !name.is_empty() && tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic())
        }
        None => false,
    }
}

/// `query`, a URL, with a scheme.
pub(super) fn normalize_url(query: &str) -> String {
    if query.starts_with("http://") || query.starts_with("https://") {
        query.to_string()
    } else {
        format!("https://{query}")
    }
}

/// `text` for a URL's query: RFC 3986's unreserved characters as they are,
/// every other byte escaped.
pub(super) fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// How long `text` says: "5m", "5 min", "1h30m", "1h 30m", "90s",
/// "2 hours", "1:30" (minutes and seconds), or a bare number of minutes.
pub(super) fn parse_duration(text: &str) -> Option<Duration> {
    let text = text.trim().to_lowercase();
    if text.is_empty() {
        return None;
    }
    if let Some((minutes, seconds)) = text.split_once(':') {
        let minutes: u64 = minutes.trim().parse().ok()?;
        let seconds: u64 = seconds.trim().parse().ok()?;
        return (seconds < 60).then(|| Duration::from_secs(minutes * 60 + seconds));
    }
    if let Ok(minutes) = text.parse::<f64>() {
        return (minutes > 0.0).then(|| Duration::from_secs_f64(minutes * 60.0));
    }
    let mut total = 0.0;
    let mut rest = text.as_str();
    while !rest.is_empty() {
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(rest.len());
        let number: f64 = rest[..end].parse().ok()?;
        rest = rest[end..].trim_start();
        let unit_end = rest
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(rest.len());
        let seconds = match &rest[..unit_end] {
            "h" | "hr" | "hrs" | "hour" | "hours" => 3600.0,
            "m" | "min" | "mins" | "minute" | "minutes" => 60.0,
            "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
            _ => return None,
        };
        total += number * seconds;
        rest = rest[unit_end..].trim_start();
    }
    (total > 0.0).then(|| Duration::from_secs_f64(total))
}

/// `length` in words: "5 min", "1 h 30 min", "45 s".
pub(super) fn format_duration(length: Duration) -> String {
    let secs = length.as_secs();
    let (hours, minutes, seconds) = (secs / 3600, secs / 60 % 60, secs % 60);
    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours} h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes} min"));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{seconds} s"));
    }
    parts.join(" ")
}

/// How long ago something happened, `seconds` since: "Now", "5 min ago",
/// "3 h ago", "2 d ago".
pub(super) fn ago(seconds: u64) -> String {
    match seconds {
        0..60 => String::from("Now"),
        60..3600 => format!("{} min ago", seconds / 60),
        3600..86400 => format!("{} h ago", seconds / 3600),
        _ => format!("{} d ago", seconds / 86400),
    }
}

/// `bytes` as a size people read: "830 bytes", "12 KB", "2.4 MB".
pub(super) fn size(bytes: usize) -> String {
    match bytes {
        0..1000 => format!("{bytes} bytes"),
        1000..1_000_000 => format!("{} KB", bytes / 1000),
        _ => format!("{:.1} MB", bytes as f64 / 1e6),
    }
}

/// How many keys an action's quick keys can be.
pub(super) const MAX_QUICK_KEYS: usize = 4;

/// Whether `keys` can be an action's quick keys: one to four letters or
/// digits.
pub(super) fn valid_quick_keys(keys: &str) -> bool {
    (1..=MAX_QUICK_KEYS).contains(&keys.chars().count())
        && keys
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn an_app_goes_under_each_of_its_categories() {
        let browser = mask(&strings(&["Network", "WebBrowser"]), "firefox %u");
        assert!(in_bucket(browser, 0));
        assert!(in_bucket(browser, 7));
        assert!(!in_bucket(browser, 2));

        let player = mask(&strings(&["AudioVideo", "Player"]), "mpv");
        assert!(in_bucket(player, 8));
    }

    #[test]
    fn web_apps_and_help_are_guessed() {
        let web_app = mask(&[], "chromium --app=https://example.com");
        assert!(in_bucket(web_app, 1));
        let help = mask(&strings(&["Core", "GNOME", "HelpViewer"]), "yelp");
        assert!(in_bucket(help, 6));
    }

    #[test]
    fn only_pills_with_apps_show() {
        let masks = [
            mask(&strings(&["Development"]), "code"),
            mask(&strings(&["Game"]), "steam"),
        ];
        assert_eq!(active_buckets(masks), vec![0, 2, 4]);
        assert_eq!(active_buckets([]), vec![0]);
    }

    #[test]
    fn durations_read_as_people_type_them() {
        let secs = |text: &str| parse_duration(text).map(|length| length.as_secs());
        assert_eq!(secs("5m"), Some(300));
        assert_eq!(secs("5 min"), Some(300));
        assert_eq!(secs("5"), Some(300));
        assert_eq!(secs("1h30m"), Some(5400));
        assert_eq!(secs("1h 30m"), Some(5400));
        assert_eq!(secs("90s"), Some(90));
        assert_eq!(secs("2 hours"), Some(7200));
        assert_eq!(secs("1:30"), Some(90));
        assert_eq!(secs("1.5h"), Some(5400));
        assert_eq!(secs("soon"), None);
        assert_eq!(secs("5 parsecs"), None);
        assert_eq!(secs("0"), None);
        assert_eq!(secs("1:75"), None);
        assert_eq!(secs(""), None);
    }

    #[test]
    fn durations_are_shown_in_words() {
        assert_eq!(format_duration(Duration::from_secs(300)), "5 min");
        assert_eq!(format_duration(Duration::from_secs(5400)), "1 h 30 min");
        assert_eq!(format_duration(Duration::from_secs(45)), "45 s");
        assert_eq!(format_duration(Duration::from_secs(3601)), "1 h 1 s");
    }

    #[test]
    fn times_and_sizes_read_as_people_say_them() {
        assert_eq!(ago(5), "Now");
        assert_eq!(ago(300), "5 min ago");
        assert_eq!(ago(7200), "2 h ago");
        assert_eq!(ago(200_000), "2 d ago");
        assert_eq!(size(830), "830 bytes");
        assert_eq!(size(12_345), "12 KB");
        assert_eq!(size(2_400_000), "2.4 MB");
    }

    #[test]
    fn quick_keys_are_a_few_letters_or_digits() {
        assert!(valid_quick_keys("nl"));
        assert!(valid_quick_keys("1"));
        assert!(valid_quick_keys("ab12"));
        assert!(!valid_quick_keys(""));
        assert!(!valid_quick_keys("abcde"));
        assert!(!valid_quick_keys("N"));
        assert!(!valid_quick_keys("a b"));
    }

    #[test]
    fn up_and_down_walk_the_searches_run_before() {
        assert_eq!(history_step(None, true, 3), Some(Some(0)));
        assert_eq!(history_step(Some(0), true, 3), Some(Some(1)));
        assert_eq!(history_step(Some(2), true, 3), None);
        assert_eq!(history_step(Some(1), false, 3), Some(Some(0)));
        assert_eq!(history_step(Some(0), false, 3), Some(None));
        assert_eq!(history_step(None, false, 3), None);
        assert_eq!(history_step(None, true, 0), None);
    }

    #[test]
    fn sums_follow_precedence_and_brackets() {
        assert_eq!(evaluate("1 + 2 * 3"), Some(7.0));
        assert_eq!(evaluate("(1 + 2) * 3"), Some(9.0));
        assert_eq!(evaluate("-4 / 2"), Some(-2.0));
        assert_eq!(evaluate("2.5*2"), Some(5.0));
    }

    #[test]
    fn plain_words_and_numbers_are_not_sums() {
        assert_eq!(evaluate("42"), None);
        assert_eq!(evaluate("firefox"), None);
        assert_eq!(evaluate("1 +"), None);
        assert_eq!(evaluate("(1 + 2"), None);
        assert_eq!(evaluate("1 / 0"), None);
        assert_eq!(evaluate("vs-code"), None);
    }

    #[test]
    fn answers_drop_trailing_zeros() {
        assert_eq!(format_number(7.0), "7");
        assert_eq!(format_number(0.5), "0.5");
        assert_eq!(format_number(1.0 / 3.0), "0.333333");
    }

    #[test]
    fn urls_need_a_scheme_or_a_domain() {
        assert!(looks_like_url("example.com"));
        assert!(looks_like_url("https://localhost:8080"));
        assert!(looks_like_url("github.com/098765434"));
        assert!(!looks_like_url("firefox"));
        assert!(!looks_like_url("1.5"));
        assert!(!looks_like_url("two words.com"));
        assert_eq!(normalize_url("example.com"), "https://example.com");
        assert_eq!(normalize_url("http://a.io"), "http://a.io");
    }

    #[test]
    fn a_query_is_escaped_for_a_url() {
        assert_eq!(percent_encode("rust lang"), "rust%20lang");
        assert_eq!(percent_encode("a&b=c"), "a%26b%3Dc");
        assert_eq!(percent_encode("é"), "%C3%A9");
    }
}
