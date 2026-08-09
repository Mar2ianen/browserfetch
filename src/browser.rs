use std::fs;
use std::path::PathBuf;

use crate::extensions::{self, Extension, Profile};
use crate::logo;
use crate::util::{command_output, home_dir};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserFamily {
    Firefox,
    Chromium,
    Chrome,
    Brave,
    Vivaldi,
    Edge,
    Opera,
    WebKit,
    Unknown,
}

struct BrowserMeta {
    family: BrowserFamily,
    engine: &'static str,
    color: &'static str,
    signatures: &'static [&'static str],
}

const BROWSER_META: &[BrowserMeta] = &[
    BrowserMeta {
        family: BrowserFamily::Firefox,
        engine: "Gecko",
        color: "208",
        signatures: &["firefox", "librewolf", "waterfox", "floorp", "zen-browser"],
    },
    BrowserMeta {
        family: BrowserFamily::Brave,
        engine: "Blink",
        color: "202",
        signatures: &["brave", "brave-browser"],
    },
    BrowserMeta {
        family: BrowserFamily::Vivaldi,
        engine: "Blink",
        color: "196",
        signatures: &["vivaldi"],
    },
    BrowserMeta {
        family: BrowserFamily::Edge,
        engine: "Blink",
        color: "45",
        signatures: &["microsoft-edge", "edge"],
    },
    BrowserMeta {
        family: BrowserFamily::Chrome,
        engine: "Blink",
        color: "34",
        signatures: &["google-chrome", "chrome"],
    },
    BrowserMeta {
        family: BrowserFamily::Opera,
        engine: "Blink",
        color: "196",
        signatures: &["opera"],
    },
    BrowserMeta {
        family: BrowserFamily::Chromium,
        engine: "Blink",
        color: "33",
        signatures: &["chromium", "ungoogled-chromium"],
    },
    BrowserMeta {
        family: BrowserFamily::WebKit,
        engine: "WebKit",
        color: "39",
        signatures: &["epiphany", "org.gnome.epiphany", "gnome-web", "surf"],
    },
];

#[derive(Debug)]
pub struct Browser {
    pub name: String,
    pub engine: String,
    pub color: &'static str,
    pub icon: Option<PathBuf>,
    pub version: Option<String>,
    pub profile_root: Option<PathBuf>,
    pub active_profile: Option<Profile>,
    pub extensions: Vec<Extension>,
}

impl Browser {
    pub fn detect() -> Self {
        let desktop_id = default_browser_desktop_id();
        let desktop = desktop_id.as_deref().and_then(read_desktop_file);
        let exec = desktop
            .as_ref()
            .and_then(|data| desktop_value(data, "Exec"))
            .map(clean_exec);
        let icon_name = desktop
            .as_ref()
            .and_then(|data| desktop_value(data, "Icon"));
        let name = desktop
            .as_ref()
            .and_then(|data| desktop_value(data, "Name"))
            .or_else(|| exec.clone())
            .unwrap_or_else(|| "Unknown browser".to_string());
        let meta = detect_meta(
            &name,
            desktop_id.as_deref(),
            exec.as_deref(),
            icon_name.as_deref(),
        );
        let family = meta
            .map(|meta| meta.family)
            .unwrap_or(BrowserFamily::Unknown);
        let profile_root = find_profile_root(family);
        let active_profile = profile_root
            .as_ref()
            .and_then(|root| extensions::active_profile(family, root));
        let extensions = profile_root
            .as_ref()
            .map(|root| extensions::browser_extensions(family, root, active_profile.as_ref()))
            .unwrap_or_default();
        let icon = icon_name.and_then(|icon| logo::resolve_icon(&icon));
        let version = exec.as_deref().and_then(browser_version);

        Self {
            name,
            engine: meta
                .map(|meta| meta.engine)
                .unwrap_or("unknown")
                .to_string(),
            color: meta.map(|meta| meta.color).unwrap_or("117"),
            icon,
            version,
            profile_root,
            active_profile,
            extensions,
        }
    }
}

fn default_browser_desktop_id() -> Option<String> {
    command_output("xdg-settings", &["get", "default-web-browser"])
        .filter(|value| !value.trim().is_empty())
}

fn read_desktop_file(id: &str) -> Option<String> {
    desktop_dirs()
        .into_iter()
        .map(|dir| dir.join(id))
        .find_map(|path| fs::read_to_string(path).ok())
}

fn desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }
    dirs.push(PathBuf::from("/usr/local/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));
    dirs
}

fn desktop_value(data: &str, key: &str) -> Option<String> {
    data.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .map(|value| value.trim().to_string())
}

fn clean_exec(exec: String) -> String {
    exec.split_whitespace()
        .find(|part| !part.starts_with('%') && !part.contains('='))
        .unwrap_or(exec.as_str())
        .to_string()
}

fn detect_meta(
    name: &str,
    desktop_id: Option<&str>,
    exec: Option<&str>,
    icon: Option<&str>,
) -> Option<&'static BrowserMeta> {
    let probe = format!(
        "{} {} {} {}",
        name.to_lowercase(),
        desktop_id.unwrap_or_default().to_lowercase(),
        exec.unwrap_or_default().to_lowercase(),
        icon.unwrap_or_default().to_lowercase()
    );
    BROWSER_META.iter().find(|meta| {
        meta.signatures
            .iter()
            .any(|signature| probe.contains(signature))
    })
}

fn find_profile_root(family: BrowserFamily) -> Option<PathBuf> {
    let home = home_dir()?;
    let config = home.join(".config");
    let candidates = match family {
        BrowserFamily::Firefox => vec![
            config.join("mozilla/firefox"),
            home.join(".mozilla/firefox"),
            home.join(".var/app/org.mozilla.firefox/.mozilla/firefox"),
            config.join("librewolf"),
            home.join(".librewolf"),
            config.join("waterfox"),
            config.join("floorp"),
            config.join("zen"),
        ],
        BrowserFamily::Chrome => vec![config.join("google-chrome")],
        BrowserFamily::Chromium => vec![config.join("chromium"), config.join("ungoogled-chromium")],
        BrowserFamily::Brave => vec![config.join("BraveSoftware/Brave-Browser")],
        BrowserFamily::Vivaldi => vec![config.join("vivaldi")],
        BrowserFamily::Edge => vec![config.join("microsoft-edge")],
        BrowserFamily::Opera => vec![config.join("opera")],
        BrowserFamily::WebKit | BrowserFamily::Unknown => Vec::new(),
    };
    candidates.into_iter().find(|path| path.exists())
}

fn browser_version(exec: &str) -> Option<String> {
    command_output(exec, &["--version"])
}
