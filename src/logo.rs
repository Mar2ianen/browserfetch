use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::util::home_dir;

pub fn render_logo(icon: Option<&Path>, label: &str) -> Vec<String> {
    icon.and_then(chafa_logo)
        .filter(|lines| !lines.is_empty())
        .unwrap_or_else(|| text_logo(label))
}

pub fn resolve_icon(icon: &str) -> Option<PathBuf> {
    let direct = PathBuf::from(icon);
    if direct.is_absolute() && direct.exists() {
        return Some(direct);
    }

    let names = if Path::new(icon).extension().is_some() {
        vec![icon.to_string()]
    } else {
        ["png", "svg", "xpm"]
            .into_iter()
            .map(|ext| format!("{icon}.{ext}"))
            .collect()
    };

    let mut roots = Vec::new();
    if let Some(home) = home_dir() {
        roots.push(home.join(".local/share/icons"));
        roots.push(home.join(".icons"));
    }
    roots.push(PathBuf::from("/usr/share/pixmaps"));
    roots.push(PathBuf::from("/usr/share/icons/hicolor"));
    roots.push(PathBuf::from("/usr/share/icons"));

    for root in roots {
        if let Some(path) = find_named_file(&root, &names, 6) {
            return Some(path);
        }
    }
    None
}

fn find_named_file(root: &Path, names: &[String], depth: usize) -> Option<PathBuf> {
    if depth == 0 || !root.is_dir() {
        return None;
    }
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .file_name()
                .and_then(OsStr::to_str)
                .map(|name| names.iter().any(|wanted| wanted == name))
                .unwrap_or(false)
        {
            return Some(path);
        }
        if path.is_dir()
            && let Some(found) = find_named_file(&path, names, depth - 1)
        {
            return Some(found);
        }
    }
    None
}

fn chafa_logo(path: &Path) -> Option<Vec<String>> {
    let output = Command::new("chafa")
        .args([
            "--symbols",
            "block",
            "--fill",
            "block",
            "--polite",
            "on",
            "--animate",
            "off",
            "--size",
            "28x14",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .replace("\u{1b}[?25l", "")
        .replace("\u{1b}[?25h", "")
        .lines()
        .map(trim_visible_end)
        .collect();
    Some(lines)
}

fn trim_visible_end(line: &str) -> String {
    let plain = strip_ansi(line);
    let trim_count = plain
        .chars()
        .rev()
        .take_while(|ch| ch.is_whitespace())
        .count();
    if trim_count == 0 {
        return line.to_string();
    }

    let mut visible_seen = 0;
    let target_visible = plain.chars().count().saturating_sub(trim_count);
    let mut cut = line.len();
    let mut chars = line.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if ch == '\u{1b}' && chars.peek().map(|(_, next)| *next) == Some('[') {
            for (_, next) in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        if visible_seen == target_visible {
            cut = idx;
            break;
        }
        visible_seen += 1;
    }

    let mut trimmed = line[..cut].to_string();
    trimmed.push_str("\x1b[0m");
    trimmed
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn text_logo(label: &str) -> Vec<String> {
    let label = label.trim();
    let width = label.len().clamp(12, 28);
    vec![
        format!("+{}+", "-".repeat(width + 2)),
        format!("| {:width$} |", label, width = width),
        format!("+{}+", "-".repeat(width + 2)),
    ]
}
