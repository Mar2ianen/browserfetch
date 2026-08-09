use crate::browser::Browser;
use crate::util::{char_width, command_output, visible_width};
use crate::{logo, os};

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const VALUE: &str = "\x1b[37m";

pub fn render(browser: &Browser) {
    let theme = Theme::new(browser.color);
    let logo = logo::render_logo(browser.icon.as_deref(), &browser.name);
    let logo_width = logo
        .iter()
        .map(|line| visible_width(line))
        .max()
        .unwrap_or(0);
    let right_width = terminal_width().saturating_sub(logo_width + 4).max(50);

    let engine = browser
        .engine
        .map(|engine| engine.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let mut info = vec![
        title_line(theme),
        separator_line(right_width),
        row("Browser", &browser.name, right_width, theme),
        row("Engine", &engine, right_width, theme),
        row(
            "Version",
            &clean_version(browser.version.as_deref().unwrap_or("unknown")),
            right_width,
            theme,
        ),
        row("OS", &os::summary(), right_width, theme),
        row("Session", &os::session_summary(), right_width, theme),
    ];

    if let Some(profile) = &browser.active_profile {
        let name = profile.name.as_deref().unwrap_or("unnamed");
        let path = profile
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown");
        info.push(row(
            "Profile",
            &format!("{name} / {path}"),
            right_width,
            theme,
        ));
    } else if let Some(backend) = &browser.profile_backend {
        info.push(row(
            "Profile",
            &backend.root().display().to_string(),
            right_width,
            theme,
        ));
    } else {
        info.push(row("Profile", "unknown", right_width, theme));
    }

    let enabled = browser
        .extensions
        .iter()
        .filter(|ext| ext.active == Some(true))
        .count();
    let disabled = browser
        .extensions
        .iter()
        .filter(|ext| ext.active == Some(false))
        .count();
    let unknown = browser
        .extensions
        .iter()
        .filter(|ext| ext.active.is_none())
        .count();
    info.push(row(
        "Extensions",
        &format!(
            "{} ({} enabled, {} disabled, {} unknown)",
            browser.extensions.len(),
            enabled,
            disabled,
            unknown
        ),
        right_width,
        theme,
    ));
    info.extend(extension_summary_rows(browser, right_width, theme));

    let rows = logo.len().max(info.len());

    for idx in 0..rows {
        let left = logo.get(idx).map(String::as_str).unwrap_or("");
        let right = info.get(idx).map(String::as_str).unwrap_or("");
        let pad = logo_width.saturating_sub(visible_width(left)) + 4;
        println!("{left}{}{right}", " ".repeat(pad));
    }
}

#[derive(Clone, Copy)]
struct Theme {
    key: &'static str,
}

impl Theme {
    fn new(color: &'static str) -> Self {
        Self {
            key: match color {
                "33" => "\x1b[38;5;33m",
                "34" => "\x1b[38;5;34m",
                "39" => "\x1b[38;5;39m",
                "45" => "\x1b[38;5;45m",
                "196" => "\x1b[38;5;196m",
                "202" => "\x1b[38;5;202m",
                "208" => "\x1b[38;5;208m",
                _ => "\x1b[38;5;117m",
            },
        }
    }
}

fn row(key: &str, value: &str, width: usize, theme: Theme) -> String {
    let key_width = 10;
    let value_width = width.saturating_sub(key_width + 3);
    format!(
        "{}{key:<key_width$}{RESET}: {VALUE}{}{RESET}",
        theme.key,
        truncate(value, value_width),
        key_width = key_width
    )
}

fn extension_summary_rows(browser: &Browser, width: usize, theme: Theme) -> Vec<String> {
    let enabled = browser
        .extensions
        .iter()
        .filter(|ext| ext.active == Some(true))
        .map(extension_label)
        .collect::<Vec<_>>();
    let disabled = browser
        .extensions
        .iter()
        .filter(|ext| ext.active == Some(false))
        .map(extension_label)
        .collect::<Vec<_>>();
    let unknown = browser
        .extensions
        .iter()
        .filter(|ext| ext.active.is_none())
        .map(extension_label)
        .collect::<Vec<_>>();

    let mut rows = Vec::new();
    if !enabled.is_empty() {
        rows.extend(extension_tree_group("Enabled", &enabled, width, theme));
    }
    if !disabled.is_empty() {
        rows.extend(extension_tree_group("Disabled", &disabled, width, theme));
    }
    if !unknown.is_empty() {
        rows.extend(extension_tree_group("Unknown", &unknown, width, theme));
    }
    rows
}

fn extension_tree_group(label: &str, items: &[String], width: usize, theme: Theme) -> Vec<String> {
    let mut rows = vec![row(label, "", width, theme)];
    for (idx, item) in items.iter().enumerate() {
        let last_item = idx + 1 == items.len();
        let branch = if last_item { "└─" } else { "├─" };
        rows.push(tree_leaf_row("", branch, item, width, theme));
    }
    rows
}

fn tree_leaf_row(key: &str, branch: &str, value: &str, width: usize, theme: Theme) -> String {
    let key_width = 10;
    let value_width = width.saturating_sub(key_width + 3);
    let prefix = format!("{branch} ");
    let value = truncate(value, value_width.saturating_sub(visible_width(&prefix)));
    format!(
        "{}{key:<key_width$}{RESET}: {VALUE}{prefix}{value}{RESET}",
        theme.key,
        key_width = key_width
    )
}

fn extension_label(ext: &crate::extensions::Extension) -> String {
    match ext.version.as_deref() {
        Some(version) => format!("{} {}", ext.name, version),
        None => ext.name.clone(),
    }
}

fn title_line(theme: Theme) -> String {
    let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let host = command_output("hostname", &["-s"]).unwrap_or_else(|| "host".to_string());
    format!(
        "{BOLD}{}{user}{RESET}{DIM}@{RESET}{BOLD}{}{host}{RESET}",
        theme.key, theme.key
    )
}

fn separator_line(width: usize) -> String {
    format!("{DIM}{}{RESET}", "-".repeat(width.min(34)))
}

fn clean_version(version: &str) -> String {
    version
        .strip_prefix("Mozilla ")
        .unwrap_or(version)
        .to_string()
}

fn truncate(value: &str, max_width: usize) -> String {
    if visible_width(value) <= max_width {
        return value.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }
    let mut out = String::new();
    let target_width = max_width - 3;
    let mut used_width = 0;
    for ch in value.chars() {
        let width = char_width(ch);
        if used_width + width > target_width {
            break;
        }
        out.push(ch);
        used_width += width;
    }
    out.push_str("...");
    out
}

fn terminal_width() -> usize {
    tty_width()
        .or_else(|| command_output("stty", &["size"]).and_then(|value| positive_width(&value)))
        .or_else(|| command_output("tput", &["cols"]).and_then(|value| positive_width(&value)))
        .or_else(|| {
            std::env::var("COLUMNS")
                .ok()
                .and_then(|value| positive_width(&value))
        })
        .unwrap_or(140)
}

#[cfg(target_os = "linux")]
fn tty_width() -> Option<usize> {
    use std::os::fd::RawFd;

    #[repr(C)]
    struct Winsize {
        rows: u16,
        columns: u16,
        x_pixels: u16,
        y_pixels: u16,
    }

    unsafe extern "C" {
        fn ioctl(fd: RawFd, request: usize, ...) -> i32;
    }

    const TIOCGWINSZ: usize = 0x5413;

    for fd in [1, 2, 0] {
        let mut size = Winsize {
            rows: 0,
            columns: 0,
            x_pixels: 0,
            y_pixels: 0,
        };
        // SAFETY: `size` is a valid writable `struct winsize` and the ioctl
        // request only fills that structure for the selected terminal fd.
        let result = unsafe { ioctl(fd, TIOCGWINSZ, &mut size) };
        if result == 0 && size.columns > 0 {
            return Some(usize::from(size.columns));
        }
    }

    None
}

#[cfg(not(target_os = "linux"))]
fn tty_width() -> Option<usize> {
    None
}

fn positive_width(value: &str) -> Option<usize> {
    value
        .split_whitespace()
        .last()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|width| *width > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_tree_keeps_group_header_above_children() {
        let items = ["First extension".to_string(), "Last extension".to_string()];
        let rows = extension_tree_group("Enabled", &items, 80, Theme::new("33"));

        assert!(rows[0].contains("Enabled"));
        assert!(!rows[0].contains("├─"));
        assert!(rows[1].contains("├─ First extension"));
        assert!(rows[2].contains("└─ Last extension"));
    }

    #[test]
    fn truncate_respects_unicode_display_width() {
        assert_eq!(truncate("日本語 browser", 12), "日本語 br...");
        assert_eq!(visible_width(&truncate("日本語 browser", 12)), 12);
    }

    #[test]
    fn parses_terminal_width_from_columns_or_stty_output() {
        assert_eq!(positive_width("120"), Some(120));
        assert_eq!(positive_width("24 120"), Some(120));
        assert_eq!(positive_width("0"), None);
    }
}
