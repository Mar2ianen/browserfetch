use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::browser::Engine;
use crate::util::{CommandSpec, home_dir};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileBackend {
    Firefox(PathBuf),
    Chromium(PathBuf),
}

impl ProfileBackend {
    pub fn root(&self) -> &Path {
        match self {
            Self::Firefox(root) | Self::Chromium(root) => root,
        }
    }

    pub fn engine(&self) -> Engine {
        match self {
            Self::Firefox(_) => Engine::Gecko,
            Self::Chromium(_) => Engine::Blink,
        }
    }

    pub fn active_profile(&self) -> Option<Profile> {
        match self {
            Self::Firefox(root) => firefox_profiles(root)
                .into_iter()
                .find(|profile| profile.is_default)
                .or_else(|| firefox_profiles(root).into_iter().next()),
            Self::Chromium(root) => chromium_active_profile(root),
        }
    }

    pub fn extensions(&self, active_profile: Option<&Profile>) -> Vec<Extension> {
        match self {
            Self::Firefox(root) => firefox_extensions(root, active_profile),
            Self::Chromium(root) => chromium_extensions(root, active_profile),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: Option<String>,
    pub path: PathBuf,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extension {
    pub name: String,
    pub version: Option<String>,
    pub id: Option<String>,
    pub active: Option<bool>,
}

pub fn discover_backend(exec: Option<&CommandSpec>) -> Option<ProfileBackend> {
    let home = home_dir()?;
    profile_root_candidates(&home, exec)
        .into_iter()
        .find_map(|root| detect_backend(root, exec))
}

fn detect_backend(root: PathBuf, exec: Option<&CommandSpec>) -> Option<ProfileBackend> {
    if root_score(&root, exec) != 0 {
        return None;
    }
    if is_firefox_profile_root(&root) {
        Some(ProfileBackend::Firefox(root))
    } else if is_chromium_profile_root(&root) {
        Some(ProfileBackend::Chromium(root))
    } else {
        None
    }
}

fn profile_root_candidates(home: &Path, exec: Option<&CommandSpec>) -> Vec<PathBuf> {
    let config = home.join(".config");
    let mut roots = vec![
        config.join("mozilla/firefox"),
        home.join(".mozilla/firefox"),
        home.join(".var/app/org.mozilla.firefox/.mozilla/firefox"),
    ];
    collect_dirs(&config, 2, &mut roots);
    collect_dirs(&home.join(".mozilla"), 2, &mut roots);
    collect_dirs(&home.join(".var/app"), 4, &mut roots);

    roots.sort_by(|left, right| {
        root_score(left, exec)
            .cmp(&root_score(right, exec))
            .then_with(|| left.cmp(right))
    });
    roots.dedup();
    roots
}

fn collect_dirs(root: &Path, depth: usize, output: &mut Vec<PathBuf>) {
    if depth == 0 || !root.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            output.push(path.clone());
            collect_dirs(&path, depth - 1, output);
        }
    }
}

fn root_score(root: &Path, exec: Option<&CommandSpec>) -> u8 {
    let Some(exec) = exec else {
        return 1;
    };
    let root = root.to_string_lossy().to_ascii_lowercase();
    let program = Path::new(&exec.program)
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if program.len() >= 4 && root.contains(&program) {
        return 0;
    }
    if exec.args.iter().any(|arg| {
        let hint = arg.to_ascii_lowercase();
        hint.len() >= 6 && root.contains(&hint)
    }) {
        return 0;
    }
    1
}

fn is_firefox_profile_root(root: &Path) -> bool {
    root.join("profiles.ini").is_file() && !firefox_profiles(root).is_empty()
}

fn is_chromium_profile_root(root: &Path) -> bool {
    root.join("Local State").is_file()
        && chromium_profiles(root).iter().any(|profile| {
            profile.path.join("Preferences").is_file() || profile.path.join("Extensions").is_dir()
        })
}

fn chromium_active_profile(root: &Path) -> Option<Profile> {
    let profiles = chromium_profiles(root);
    let last_used = read_json(&root.join("Local State")).and_then(|json| {
        json.pointer("/profile/last_used")?
            .as_str()
            .map(str::to_string)
    });

    if let Some(last_used) = last_used {
        profiles
            .iter()
            .find(|profile| profile.name.as_deref() == Some(last_used.as_str()))
            .cloned()
            .or_else(|| profiles.into_iter().next())
    } else {
        profiles.into_iter().next()
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
        let states = chromium_extension_states(&profile.path);
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
            let Some(version_dir) = newest_extension_version_dir(&id_path) else {
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
            let active = id
                .as_ref()
                .and_then(|extension_id| states.get(extension_id).copied());
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

fn chromium_extension_states(profile: &Path) -> BTreeMap<String, bool> {
    let Some(settings) = read_json(&profile.join("Preferences"))
        .and_then(|json| json.pointer("/extensions/settings").cloned())
        .and_then(|value| value.as_object().cloned())
    else {
        return BTreeMap::new();
    };

    settings
        .into_iter()
        .filter_map(|(id, value)| {
            let state = value.get("state").and_then(Value::as_i64)?;
            match state {
                0 => Some((id, false)),
                1 => Some((id, true)),
                _ => None,
            }
        })
        .collect()
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

fn newest_extension_version_dir(path: &Path) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(path)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort_by(|left, right| compare_extension_versions(left, right));
    dirs.pop()
}

fn compare_extension_versions(left: &Path, right: &Path) -> Ordering {
    match (extension_version_key(left), extension_version_key(right)) {
        (Some(left_version), Some(right_version)) => left_version
            .cmp(&right_version)
            .then_with(|| left.cmp(right)),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => left.cmp(right),
    }
}

fn extension_version_key(path: &Path) -> Option<Vec<u64>> {
    let data = fs::read_to_string(path.join("manifest.json")).ok()?;
    let json = serde_json::from_str::<Value>(&data).ok()?;
    let version = json_string(&json, "version")?;
    Some(
        version
            .split(|ch: char| !ch.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse().ok())
            .collect(),
    )
}

fn read_json(path: &Path) -> Option<Value> {
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("browserfetch-test-{stamp}"));
        fs::create_dir_all(&path).expect("create test directory");
        path
    }

    fn write_json(path: &Path, data: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(path, data).expect("write test JSON");
    }

    #[test]
    fn chromium_uses_last_used_profile() {
        let root = temp_root();
        fs::create_dir(root.join("Default")).expect("create default profile");
        fs::create_dir(root.join("Profile 2")).expect("create second profile");
        write_json(
            &root.join("Local State"),
            r#"{"profile":{"last_used":"Profile 2"}}"#,
        );

        let profile = chromium_active_profile(&root).expect("active profile");
        assert_eq!(profile.name.as_deref(), Some("Profile 2"));

        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn chromium_reads_state_and_numeric_extension_versions() {
        let root = temp_root();
        let profile = Profile {
            name: Some("Default".to_string()),
            path: root.join("Default"),
            is_default: true,
        };
        fs::create_dir(&profile.path).expect("create profile");
        write_json(
            &profile.path.join("Preferences"),
            r#"{"extensions":{"settings":{"disabled":{"state":0},"enabled":{"state":1}}}}"#,
        );
        write_json(
            &profile.path.join("Extensions/disabled/9.9.0/manifest.json"),
            r#"{"name":"Disabled","version":"9.9.0"}"#,
        );
        write_json(
            &profile
                .path
                .join("Extensions/disabled/10.0.0/manifest.json"),
            r#"{"name":"Disabled","version":"10.0.0"}"#,
        );
        write_json(
            &profile.path.join("Extensions/enabled/1.0.0/manifest.json"),
            r#"{"name":"Enabled","version":"1.0.0"}"#,
        );

        let extensions = chromium_extensions(&root, Some(&profile));
        let disabled = extensions
            .iter()
            .find(|extension| extension.name == "Disabled")
            .expect("disabled extension");
        let enabled = extensions
            .iter()
            .find(|extension| extension.name == "Enabled")
            .expect("enabled extension");
        assert_eq!(disabled.version.as_deref(), Some("10.0.0"));
        assert_eq!(disabled.active, Some(false));
        assert_eq!(enabled.active, Some(true));

        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn executable_hint_prioritizes_matching_profile_root() {
        let exec = CommandSpec {
            program: "/usr/lib/firefox/firefox".to_string(),
            args: Vec::new(),
            env: Vec::new(),
        };
        let firefox_root = Path::new("/tmp/.config/mozilla/firefox");
        let chromium_root = Path::new("/tmp/.config/google-chrome");
        assert!(root_score(firefox_root, Some(&exec)) < root_score(chromium_root, Some(&exec)));
    }

    #[test]
    fn unrelated_profile_root_is_not_used_as_backend() {
        let root = temp_root();
        fs::create_dir(root.join("profile")).expect("create Firefox profile");
        fs::write(
            root.join("profiles.ini"),
            "[Profile0]\nName=default\nIsRelative=1\nPath=profile\n",
        )
        .expect("write profiles.ini");
        let exec = CommandSpec {
            program: "/usr/bin/foo-browser".to_string(),
            args: Vec::new(),
            env: Vec::new(),
        };

        assert_eq!(detect_backend(root.clone(), Some(&exec)), None);
        fs::remove_dir_all(root).expect("remove test directory");
    }
}
