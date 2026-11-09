//! niri's keyboard shortcuts, read from its `binds` blocks for the
//! Keyboard Shortcuts list. niri's IPC doesn't report them, so they're
//! read from `config.kdl` and the files it includes, in order: a key bound
//! again later is the later bind, as niri has it. Nothing here writes.

use std::fs;
use std::path::{Path, PathBuf};

use crate::niri_files::{code, niri_config, node_end, spans, unquote};

/// What a list groups shortcuts under, in the order shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Shell,
    Apps,
    Windows,
    Workspaces,
    Monitors,
    Screenshots,
    Hardware,
    Session,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::Shell,
        Category::Apps,
        Category::Windows,
        Category::Workspaces,
        Category::Monitors,
        Category::Screenshots,
        Category::Hardware,
        Category::Session,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Category::Shell => "Shell",
            Category::Apps => "Apps",
            Category::Windows => "Windows and Columns",
            Category::Workspaces => "Workspaces",
            Category::Monitors => "Monitors",
            Category::Screenshots => "Screenshots",
            Category::Hardware => "Media and Hardware",
            Category::Session => "Session",
        }
    }
}

/// One shortcut.
#[derive(Clone, Debug, PartialEq)]
pub struct Bind {
    /// As written: `Mod+Shift+Slash`.
    pub keys: String,
    /// Its `hotkey-overlay-title`, else what it does in words.
    pub label: String,
    /// niri's action: `spawn`, `focus-column-left`.
    pub action: String,
    /// What it runs, for `spawn` and `spawn-sh`.
    pub command: Option<String>,
    pub when_locked: bool,
    pub category: Category,
}

impl Bind {
    /// The keys as people read them: `Super+Shift+/`.
    pub fn shown_keys(&self) -> String {
        self.keys
            .split('+')
            .map(key_name)
            .collect::<Vec<_>>()
            .join("+")
    }

    /// Whether `query` (lower case) finds it.
    pub fn matches(&self, query: &str) -> bool {
        [
            self.label.as_str(),
            &self.keys,
            &self.shown_keys(),
            &self.action,
            self.command.as_deref().unwrap_or(""),
        ]
        .iter()
        .any(|text| text.to_lowercase().contains(query))
    }
}

fn key_name(key: &str) -> String {
    let named = match key {
        "Mod" | "Super" | "Win" => "Super",
        "Ctrl" | "Control" => "Ctrl",
        "Slash" => "/",
        "Backslash" => "\\",
        "Comma" => ",",
        "Period" => ".",
        "Minus" => "-",
        "Equal" => "=",
        "Plus" => "+",
        "Grave" => "`",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Semicolon" => ";",
        "Apostrophe" => "'",
        "Print" => "Print Screen",
        "Return" => "Return",
        "WheelScrollDown" => "Scroll Down",
        "WheelScrollUp" => "Scroll Up",
        "WheelScrollLeft" => "Scroll Left",
        "WheelScrollRight" => "Scroll Right",
        _ => "",
    };
    if !named.is_empty() {
        return named.to_string();
    }
    // XF86AudioRaiseVolume: Audio Raise Volume
    if let Some(rest) = key.strip_prefix("XF86") {
        return words(rest);
    }
    // Page_Down: Page Down
    key.replace('_', " ")
}

/// `AudioRaiseVolume` as `Audio Raise Volume`.
fn words(camel: &str) -> String {
    let mut out = String::new();
    for (index, c) in camel.chars().enumerate() {
        if index > 0 && c.is_uppercase() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

// ───── reading ──────────────────────────────────────────────────────────────

/// Every shortcut niri's config sets, and the files they came from; none
/// when there's no config.
pub fn read() -> Vec<Bind> {
    let config = niri_config();
    let mut texts = Vec::new();
    gather(&config, &mut texts, 0);
    merge(texts.iter().map(String::as_str))
}

/// `path`'s text, then each file it includes, in the order niri reads them.
fn gather(path: &Path, texts: &mut Vec<String>, depth: usize) {
    if depth > 8 {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let folder = include_folder(path);
    let includes: Vec<PathBuf> = text
        .lines()
        .filter_map(|line| include_path(code(line).trim()))
        .map(|file| folder.join(file))
        .collect();
    texts.push(text);
    for file in includes {
        gather(&file, texts, depth + 1);
    }
}

/// Where a relative include is read from: niri's config folder, the one
/// the symlink is in, not the one it leads to.
fn include_folder(path: &Path) -> PathBuf {
    let link = emaki_kit::paths::config().join("niri");
    if path == niri_config() {
        link
    } else {
        path.parent().map(Path::to_path_buf).unwrap_or(link)
    }
}

/// The file of `include "…"` or `include optional=true "…"`.
fn include_path(code: &str) -> Option<String> {
    let rest = code.strip_prefix("include ")?;
    let quoted = rest.rsplit_once(' ').map_or(rest, |(_, last)| last);
    unquote(quoted)
}

/// The binds of `texts` read in order, a key bound again replacing its
/// earlier bind where it was.
pub fn merge<'a>(texts: impl Iterator<Item = &'a str>) -> Vec<Bind> {
    let mut all: Vec<Bind> = Vec::new();
    for text in texts {
        for bind in binds_in(text) {
            let key = bind.keys.to_lowercase();
            match all.iter_mut().find(|b| b.keys.to_lowercase() == key) {
                Some(old) => *old = bind,
                None => all.push(bind),
            }
        }
    }
    all
}

/// The binds of one file's `binds` blocks.
fn binds_in(text: &str) -> Vec<Bind> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (first, last) in spans(text, "binds") {
        let inner = &lines[first + 1..last];
        let mut at = 0;
        while at < inner.len() {
            let end = node_end(inner, at);
            let node: Vec<&str> = inner[at..=end].iter().map(|line| code(line)).collect();
            at = end + 1;
            let joined = node.join(" ");
            if let Some(bind) = parse_bind(&joined) {
                out.push(bind);
            }
        }
    }
    out
}

#[derive(Debug, PartialEq)]
enum Token {
    Word(String),
    Text(String),
    Prop(String, String),
    Open,
    Close,
    End,
}

/// KDL's pieces as far as a bind needs: words, strings with their
/// escapes, `name=value`, braces and `;`.
fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    let string = |chars: &mut std::iter::Peekable<std::str::Chars>| {
        let mut value = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some(other) => value.push(other),
                    None => {}
                },
                '"' => break,
                _ => value.push(c),
            }
        }
        value
    };
    while let Some(&c) = chars.peek() {
        match c {
            _ if c.is_whitespace() => {
                chars.next();
            }
            '{' => {
                chars.next();
                out.push(Token::Open);
            }
            '}' => {
                chars.next();
                out.push(Token::Close);
            }
            ';' => {
                chars.next();
                out.push(Token::End);
            }
            '"' => {
                chars.next();
                out.push(Token::Text(string(&mut chars)));
            }
            _ => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || "{};\"".contains(c) {
                        break;
                    }
                    word.push(c);
                    chars.next();
                }
                if let Some(name) = word.strip_suffix('=') {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        out.push(Token::Prop(name.to_string(), string(&mut chars)));
                    } else {
                        out.push(Token::Prop(name.to_string(), String::new()));
                    }
                } else if let Some((name, value)) = word.split_once('=') {
                    out.push(Token::Prop(name.to_string(), value.to_string()));
                } else {
                    out.push(Token::Word(word));
                }
            }
        }
    }
    out
}

/// `KEYS props { action args; }`, or none for a line that's no bind.
fn parse_bind(text: &str) -> Option<Bind> {
    let tokens = tokens(text);
    let mut it = tokens.into_iter();
    let keys = match it.next()? {
        Token::Word(keys) if !keys.starts_with("/-") => keys,
        _ => return None,
    };
    let mut title = None;
    let mut when_locked = false;
    for token in it.by_ref() {
        match token {
            Token::Open => break,
            Token::Prop(name, value) => match name.as_str() {
                "hotkey-overlay-title" if value != "null" && !value.is_empty() => {
                    title = Some(value)
                }
                "allow-when-locked" => when_locked = value == "true",
                _ => {}
            },
            _ => return None,
        }
    }
    let action = match it.next()? {
        Token::Word(action) => action,
        _ => return None,
    };
    let mut args = Vec::new();
    for token in it {
        match token {
            Token::Text(text) => args.push(text),
            Token::Word(word) => args.push(word),
            Token::End | Token::Close => break,
            _ => {}
        }
    }
    let command = match action.as_str() {
        "spawn" => Some(spawn_command(&args)),
        "spawn-sh" => args.first().cloned(),
        _ => None,
    };
    let category = category(&keys, &action, command.as_deref());
    let label = title.unwrap_or_else(|| label(&action, command.as_deref(), &args));
    Some(Bind {
        keys,
        label,
        action,
        command,
        when_locked,
        category,
    })
}

/// A `spawn`'s words as a command line; `sh -c "…"` as what it runs.
fn spawn_command(args: &[String]) -> String {
    match args {
        [shell, flag, script, ..] if (shell == "sh" || shell == "bash") && flag == "-c" => {
            script.clone()
        }
        _ => args.join(" "),
    }
}

/// What the emaki shell is asked over its socket: `printf 'osd\nvolume\nup\n'`
/// as `osd volume up`.
fn shell_request(command: &str) -> Option<String> {
    if !command.contains(".sock") {
        return None;
    }
    let start = command.find("printf '")? + "printf '".len();
    let rest = &command[start..];
    let request = &rest[..rest.find('\'')?];
    let words: Vec<&str> = request
        .split("\\n")
        .flat_map(|part| part.split('\n'))
        .filter(|part| !part.is_empty())
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

fn category(keys: &str, action: &str, command: Option<&str>) -> Category {
    let key = keys.rsplit('+').next().unwrap_or(keys);
    if let Some(command) = command {
        return if command.contains("emaki") {
            Category::Shell
        } else if key.starts_with("XF86") {
            Category::Hardware
        } else {
            Category::Apps
        };
    }
    if action.starts_with("screenshot") {
        Category::Screenshots
    } else if key.starts_with("XF86") {
        Category::Hardware
    } else if action.contains("monitor") {
        Category::Monitors
    } else if action.contains("workspace") || action == "toggle-overview" {
        Category::Workspaces
    } else if matches!(
        action,
        "quit" | "show-hotkey-overlay" | "toggle-keyboard-shortcuts-inhibit"
    ) {
        Category::Session
    } else {
        Category::Windows
    }
}

/// What a bind does, when it has no title: the shell's request, the
/// command, or niri's action in words with its argument.
fn label(action: &str, command: Option<&str>, args: &[String]) -> String {
    if let Some(command) = command {
        if let Some(request) = shell_request(command) {
            return format!("Shell: {request}");
        }
        let short: String = command.chars().take(48).collect();
        return if short.len() < command.len() {
            format!("Run {short}…")
        } else {
            format!("Run {short}")
        };
    }
    let mut words = action.replace('-', " ");
    if let Some(first) = words.get(..1) {
        words = first.to_uppercase() + &words[1..];
    }
    match args.first() {
        Some(arg) => format!("{words} {arg}"),
        None => words,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // lines as config.kdl has them, escapes and all
    const BINDS: &str = r#"input {
    keyboard { numlock; }
}

binds {
    Mod+Shift+Slash { show-hotkey-overlay; }

    // the terminal
    Mod+T hotkey-overlay-title="Open a Terminal" { spawn-sh "xdg-terminal-exec || foot"; }
    Mod+Space hotkey-overlay-title="App Launcher: emaki" { spawn "sh" "-c" "printf 'launcher\\ntoggle\\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock"; }
    Super+Alt+S allow-when-locked=true hotkey-overlay-title=null { spawn-sh "pkill orca || exec orca"; }
    XF86AudioRaiseVolume allow-when-locked=true { spawn-sh "[ \"$(printf 'osd\\nvolume\\nup\\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock)\" = ok ] || wpctl set-volume @DEFAULT_AUDIO_SINK@ 0.05+ -l 1.0"; }
    XF86AudioPlay        allow-when-locked=true { spawn-sh "playerctl play-pause"; }
    Mod+Left  { focus-column-left; }
    Mod+1 { focus-workspace 1; }
    Mod+Shift+Ctrl+Left  { move-column-to-monitor-left; }
    Print { screenshot; }
    Mod+Shift+E { quit; }
    /-Mod+X { close-window; }
    // Mod+Q { close-window; }
    Mod+R {
        switch-preset-column-width
    }
}
"#;

    fn find<'a>(binds: &'a [Bind], keys: &str) -> &'a Bind {
        binds
            .iter()
            .find(|b| b.keys == keys)
            .unwrap_or_else(|| panic!("no {keys} in {binds:#?}"))
    }

    #[test]
    fn every_bind_reads() {
        let binds = binds_in(BINDS);
        assert_eq!(binds.len(), 12, "{binds:#?}");
        let terminal = find(&binds, "Mod+T");
        assert_eq!(terminal.label, "Open a Terminal");
        assert_eq!(
            terminal.command.as_deref(),
            Some("xdg-terminal-exec || foot")
        );
        assert_eq!(terminal.category, Category::Apps);

        let launcher = find(&binds, "Mod+Space");
        assert_eq!(launcher.category, Category::Shell);
        assert!(
            launcher
                .command
                .as_deref()
                .unwrap()
                .starts_with("printf 'launcher")
        );

        let orca = find(&binds, "Super+Alt+S");
        assert!(orca.when_locked);
        assert_eq!(orca.label, "Run pkill orca || exec orca");

        let volume = find(&binds, "XF86AudioRaiseVolume");
        assert_eq!(volume.label, "Shell: osd volume up");
        assert_eq!(volume.category, Category::Shell);
        assert_eq!(find(&binds, "XF86AudioPlay").category, Category::Hardware);

        assert_eq!(find(&binds, "Mod+Left").label, "Focus column left");
        assert_eq!(find(&binds, "Mod+1").label, "Focus workspace 1");
        assert_eq!(find(&binds, "Mod+1").category, Category::Workspaces);
        assert_eq!(
            find(&binds, "Mod+Shift+Ctrl+Left").category,
            Category::Monitors
        );
        assert_eq!(find(&binds, "Print").category, Category::Screenshots);
        assert_eq!(find(&binds, "Mod+Shift+E").category, Category::Session);
        assert_eq!(find(&binds, "Mod+R").action, "switch-preset-column-width");
        assert!(binds.iter().all(|b| b.keys != "Mod+X" && b.keys != "Mod+Q"));
    }

    #[test]
    fn a_key_bound_again_later_is_the_later_bind() {
        let later = "binds {\n    mod+t { spawn \"kitty\"; }\n}\n";
        let binds = merge([BINDS, later].into_iter());
        assert_eq!(binds.len(), 12);
        assert_eq!(find(&binds, "mod+t").command.as_deref(), Some("kitty"));
    }

    #[test]
    fn keys_read_as_people_say_them() {
        let bind = find(&binds_in(BINDS), "Mod+Shift+Slash").clone();
        assert_eq!(bind.shown_keys(), "Super+Shift+/");
        assert_eq!(key_name("XF86AudioRaiseVolume"), "Audio Raise Volume");
        assert!(bind.matches("hotkey"));
        assert!(bind.matches("super+shift"));
    }

    #[test]
    fn includes_are_followed() {
        assert_eq!(
            include_path(r#"include optional=true "/a/screens.kdl""#).as_deref(),
            Some("/a/screens.kdl")
        );
        assert_eq!(
            include_path(r#"include "binds.kdl""#).as_deref(),
            Some("binds.kdl")
        );
        assert_eq!(include_path("includes 3"), None);
    }

    #[test]
    #[ignore = "reads this machine's niri config"]
    fn the_live_config_reads() {
        let binds = read();
        assert!(binds.len() > 50, "{}", binds.len());
        for bind in &binds {
            println!(
                "{:<12} {:<28} {}",
                format!("{:?}", bind.category),
                bind.keys,
                bind.label
            );
        }
    }
}
