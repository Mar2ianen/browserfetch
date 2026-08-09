use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

pub fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

pub fn command_spec_output(spec: &CommandSpec, extra_args: &[&str]) -> Option<String> {
    let output = Command::new(&spec.program)
        .args(&spec.args)
        .args(extra_args)
        .envs(
            spec.env
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        )
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

pub fn visible_width(text: &str) -> usize {
    let mut width = 0;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        width += char_width(ch);
    }
    width
}

pub fn char_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

pub fn parse_exec(value: &str) -> Option<CommandSpec> {
    let tokens = tokenize_exec(value)
        .into_iter()
        .filter(|token| !token.is_empty() && !token.contains('%'))
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return None;
    }

    let mut index = 0;
    let mut env = Vec::new();
    if is_program(&tokens[0], "env") {
        index += 1;
        while let Some(token) = tokens.get(index) {
            let Some((key, value)) = token.split_once('=') else {
                break;
            };
            if !is_environment_key(key) {
                break;
            }
            env.push((key.to_string(), value.to_string()));
            index += 1;
        }
    }

    let program = tokens.get(index)?.clone();
    let args = tokens.into_iter().skip(index + 1).collect();
    Some(CommandSpec { program, args, env })
}

fn tokenize_exec(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;

    for ch in value.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' && quote != Some('\'') {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            ch if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            ch => current.push(ch),
        }
    }

    if escaped {
        current.push('\\');
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn is_environment_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn is_program(path: &str, expected: &str) -> bool {
    Path::new(path)
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name == expected)
}

pub fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_desktop_exec_field_codes() {
        let spec = parse_exec("/usr/bin/super-browser --new-window %U").expect("command spec");
        assert_eq!(spec.program, "/usr/bin/super-browser");
        assert_eq!(spec.args, vec!["--new-window"]);
        assert!(spec.env.is_empty());
    }

    #[test]
    fn unwraps_env_and_preserves_flatpak_run_command() {
        let env_spec = parse_exec("env MOZ_ENABLE_WAYLAND=1 firefox %u").expect("env command spec");
        assert_eq!(env_spec.program, "firefox");
        assert_eq!(env_spec.args, Vec::<String>::new());
        assert_eq!(
            env_spec.env,
            vec![("MOZ_ENABLE_WAYLAND".to_string(), "1".to_string())]
        );

        let flatpak_spec =
            parse_exec("/usr/bin/flatpak run org.mozilla.firefox %U").expect("flatpak spec");
        assert_eq!(flatpak_spec.program, "/usr/bin/flatpak");
        assert_eq!(
            flatpak_spec.args,
            vec!["run".to_string(), "org.mozilla.firefox".to_string()]
        );
    }

    #[test]
    fn measures_unicode_terminal_width() {
        assert_eq!(visible_width("a中e\u{0301}"), 4);
    }
}
