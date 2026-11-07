//! A `--flag value` from the command line.

/// The value after `name` in `args`, as `--name value` or `--name=value`;
/// the first if it's given twice.
pub fn flag(args: &[String], name: &str) -> Option<String> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next().cloned();
        }
        if let Some(value) = arg
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('='))
        {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &str) -> Vec<String> {
        line.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn both_spellings_are_read() {
        assert_eq!(
            flag(&args("--page dock"), "--page").as_deref(),
            Some("dock")
        );
        assert_eq!(
            flag(&args("x --page=dock"), "--page").as_deref(),
            Some("dock")
        );
        assert_eq!(flag(&args("--pages=dock"), "--page"), None);
        assert_eq!(flag(&args("--page"), "--page"), None);
    }
}
