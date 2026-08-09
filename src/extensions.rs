use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::browser::BrowserFamily;

#[derive(Debug, Clone)]
pub struct Profile {
    pub name: Option<String>,
    pub path: PathBuf,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct Extension {
    pub name: String,
    pub version: Option<String>,
    pub id: Option<String>,
    pub active: Option<bool>,
}

pub fn active_profile(kind: BrowserFamily, root: &Path) -> Option<Profile> {
    match kind {
        BrowserFamily::Firefox => firefox_profiles(root)
            .into_iter()
            .find(|profile| profile.is_default)
            .or_else(|| firefox_profiles(root).into_iter().next()),
        BrowserFamily::Chromium
        | BrowserFamily::Chrome
        | BrowserFamily::Brave
        | BrowserFamily::Vivaldi
        | BrowserFamily::Edge
        | BrowserFamily::Opera => chromium_profiles(root).into_iter().next(),
        BrowserFamily::WebKit | BrowserFamily::Unknown => None,
    }
}

pub fn browser_extensions(
    kind: BrowserFamily,
    root: &Path,
    active_profile: Option<&Profile>,
) -> Vec<Extension> {
    match kind {
        BrowserFamily::Firefox => firefox_extensions(root, active_profile),
        BrowserFamily::Chromium
        | BrowserFamily::Chrome
        | BrowserFamily::Brave
        | BrowserFamily::Vivaldi
        | BrowserFamily::Edge
        | BrowserFamily::Opera => chromium_extensions(root, active_profile),
        BrowserFamily::WebKit | BrowserFamily::Unknown => Vec::new(),
    }
}

fn chromium_profiles(root: &Path) -> Vec<Profile> {
    let mut profiles: Vec<Profile> = fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .map(|name| name == "Default" || name.starts_with("Profile "))
                    .unwrap_or(false)
        })
        .map(|path| Profile {
            name: path.file_name().and_then(OsStr::to_str).map(str::to_string),
            is_default: path.file_name().and_then(OsStr::to_str) == Some("Default"),
            path,
        })
        .collect();
    profiles.sort_by_key(|profile| (!profile.is_default, profile.path.clone()));
    profiles
}

fn chromium_extensions(root: &Path, active_profile: Option<&Profile>) -> Vec<Extension> {
    let profiles = active_profile
        .map(|profile| vec![profile.clone()])
        .unwrap_or_else(|| chromium_profiles(root));
    let mut extensions = Vec::new();

    for profile in profiles {
        let ext_root = profile.path.join("Extensions");
        let Ok(ids) = fs::read_dir(ext_root) else {
            continue;
        };
        for id_entry in ids.flatten() {
            let id_path = id_entry.path();
            if !id_path.is_dir() {
                continue;
            }
            let id = id_path
                .file_name()
                .and_then(OsStr::to_str)
                .map(ToString::to_string);
            let Some(version_dir) = newest_child_dir(&id_path) else {
                continue;
            };
            let manifest = version_dir.join("manifest.json");
            let Ok(data) = fs::read_to_string(manifest) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<Value>(&data) else {
                continue;
            };
            let name = json_string(&json, "name")
                .filter(|name| !name.starts_with("__MSG_"))
                .or_else(|| id.clone())
                .unwrap_or_else(|| "Unknown Chromium extension".to_string());
            let version = json_string(&json, "version");
            extensions.push(Extension {
                name,
                version,
                id,
                active: None,
            });
        }
    }

    sort_extensions(extensions)
}

fn firefox_extensions(root: &Path, active_profile: Option<&Profile>) -> Vec<Extension> {
    let profiles = active_profile
        .map(|profile| vec![profile.clone()])
        .unwrap_or_else(|| firefox_profiles(root));
    let mut extensions = Vec::new();

    for profile in profiles {
        let data_path = profile.path.join("extensions.json");
        let Ok(data) = fs::read_to_string(data_path) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        let Some(addons) = json.get("addons").and_then(Value::as_array) else {
            continue;
        };
        for addon in addons {
            if addon.get("type").and_then(Value::as_str) != Some("extension") {
                continue;
            }
            if addon
                .get("hidden")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                continue;
            }
            if addon.get("visible").and_then(Value::as_bool) == Some(false) {
                continue;
            }

            let name = addon
                .get("defaultLocale")
                .and_then(|locale| json_string(locale, "name"))
                .or_else(|| json_string(addon, "id"))
                .unwrap_or_else(|| "Unknown Firefox extension".to_string());
            let version = json_string(addon, "version");
            let id = json_string(addon, "id");
            let active = addon.get("active").and_then(Value::as_bool);
            extensions.push(Extension {
                name,
                version,
                id,
                active,
            });
        }
    }

    sort_extensions(extensions)
}

fn firefox_profiles(root: &Path) -> Vec<Profile> {
    let profiles_ini = root.join("profiles.ini");
    let Ok(data) = fs::read_to_string(profiles_ini) else {
        return Vec::new();
    };
    let install_default = install_default_path(&data);
    let mut profiles = Vec::new();
    let mut section: BTreeMap<String, String> = BTreeMap::new();

    for line in data.lines().chain(std::iter::once("")) {
        let line = line.trim();
        if line.starts_with('[') || line.is_empty() {
            if let Some(path) = section.get("Path") {
                let is_relative = section
                    .get("IsRelative")
                    .map(|value| value == "1")
                    .unwrap_or(true);
                let full_path = if is_relative {
                    root.join(path)
                } else {
                    PathBuf::from(path)
                };
                if full_path.exists() {
                    let is_default = section
                        .get("Default")
                        .map(|value| value == "1")
                        .unwrap_or(false)
                        || install_default.as_deref() == Some(path.as_str());
                    profiles.push(Profile {
                        name: section.get("Name").cloned(),
                        path: full_path,
                        is_default,
                    });
                }
            }
            section.clear();
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            section.insert(key.to_string(), value.to_string());
        }
    }

    profiles.sort_by_key(|profile| (!profile.is_default, profile.path.clone()));
    profiles
}

fn install_default_path(profiles_ini: &str) -> Option<String> {
    let mut in_install = false;
    for line in profiles_ini.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_install = line.starts_with("[Install");
            continue;
        }
        if in_install && let Some(value) = line.strip_prefix("Default=") {
            return Some(value.to_string());
        }
    }
    None
}

fn newest_child_dir(path: &Path) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(path)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs.pop()
}

fn json_string(json: &Value, key: &str) -> Option<String> {
    json.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn sort_extensions(mut extensions: Vec<Extension>) -> Vec<Extension> {
    extensions.sort_by_key(|ext| ext.name.to_lowercase());
    extensions.dedup_by(|a, b| a.name == b.name && a.id == b.id);
    extensions
}
