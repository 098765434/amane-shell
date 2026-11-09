//! Text that goes into a Nix expression, quoted so it stays the text it is.

/// One part of an attribute path as Nix reads it: as is when it's a plain name (letters,
/// digits, `_`, `-`, `'`, not starting with a digit, `-` or `'`), else a quoted string with
/// `\`, `"` and `${` escaped, so no part can end the string or splice in code.
pub fn attr_part(part: &str) -> String {
    let plain = part
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '\''));
    if plain {
        return part.to_string();
    }

    let mut quoted = String::with_capacity(part.len() + 2);
    quoted.push('"');
    let mut letters = part.chars().peekable();
    while let Some(letter) = letters.next() {
        match letter {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            '$' if letters.peek() == Some(&'{') => quoted.push_str("\\$"),
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::attr_part;

    #[test]
    fn plain_names_stay_as_they_are() {
        assert_eq!(attr_part("ripgrep"), "ripgrep");
        assert_eq!(attr_part("nixos-niri"), "nixos-niri");
        assert_eq!(attr_part("_private'"), "_private'");
    }

    #[test]
    fn anything_else_is_a_quoted_string() {
        assert_eq!(attr_part("2048-game"), "\"2048-game\"");
        assert_eq!(attr_part("my host"), "\"my host\"");
        assert_eq!(attr_part(""), "\"\"");
    }

    #[test]
    fn nothing_can_end_the_string_or_splice_code() {
        assert_eq!(attr_part("a\"b"), "\"a\\\"b\"");
        assert_eq!(
            attr_part("${builtins.abort 1}"),
            "\"\\${builtins.abort 1}\""
        );
        assert_eq!(attr_part("a\\${x}"), "\"a\\\\\\${x}\"");
        assert_eq!(attr_part("a\nb"), "\"a\\nb\"");
        // a lone $ is just a letter
        assert_eq!(attr_part("$x"), "\"$x\"");
    }
}
