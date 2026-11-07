//! The keyboard layouts niri has and the one in use, for the bar's input menu: amane's
//! `KeyboardLayouts`, which follows niri's shared event stream, and the bar's short code for a
//! layout.

pub use amane::KeyboardLayouts as KeyboardLayout;

/// Switches to the layout at `index`; niri's answer comes off the UI thread.
pub fn switch(index: usize) {
    let Ok(index) = u8::try_from(index) else {
        return;
    };
    std::thread::spawn(move || {
        if let Err(err) = amane::niri::switch_keyboard_layout(index) {
            eprintln!("amane-shell: couldn't switch the keyboard layout: {err}");
        }
    });
}

/// A short code for a layout's name, as the bar shows the input source: the
/// language's own two letters where the name says, else its first two.
pub fn short(name: &str) -> String {
    const KNOWN: [(&str, &str); 14] = [
        ("English (US", "US"),
        ("English (UK", "GB"),
        ("Norwegian", "NO"),
        ("Swedish", "SE"),
        ("Danish", "DK"),
        ("German", "DE"),
        ("French", "FR"),
        ("Spanish", "ES"),
        ("Italian", "IT"),
        ("Finnish", "FI"),
        ("Dutch", "NL"),
        ("Polish", "PL"),
        ("Russian", "RU"),
        ("Japanese", "JP"),
    ];
    if let Some((_, code)) = KNOWN.iter().find(|(known, _)| name.starts_with(known)) {
        return String::from(*code);
    }
    name.chars()
        .filter(char::is_ascii_alphabetic)
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layout_has_a_short_code() {
        assert_eq!(short("Norwegian"), "NO");
        assert_eq!(short("English (US, intl., with dead keys)"), "US");
        assert_eq!(short("Klingon"), "KL");
    }
}
