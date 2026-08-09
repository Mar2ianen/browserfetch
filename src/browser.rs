use std::collections::HashSet;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::extensions::{self, Extension, Profile, ProfileBackend};
use crate::logo;
use crate::util::{CommandSpec, command_output, command_spec_output, home_dir, parse_exec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Gecko,
    Blink,
    WebKit,
    Servo,
}

impl Engine {
    pub const fn color(self) -> &'static str {
        match self {
            Self::Gecko => "208",
            Self::Blink => "33",
            Self::WebKit => "39",
            Self::Servo => "201",
        }
    }
}

impl fmt::Display for Engine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Gecko => "Gecko",
            Self::Blink => "Blink",
            Self::WebKit => "WebKit",
            Self::Servo => "Servo",
        };
        formatter.write_str(name)
    }
}

#[derive(Debug, Clone)]
struct DesktopEntry {
    id: String,
    name: Option<String>,
    exec: Option<CommandSpec>,
    icon: Option<String>,
    categories: Option<String>,
}

impl DesktopEntry {
    fn matches_selector(&self, selector: &str) -> bool {
        let selector = selector_key(selector);
        if selector.is_empty() {
            return false;
        }

        [
            Some(self.id.as_str()),
            self.name.as_deref(),
            self.exec.as_ref().map(|spec| spec.program.as_str()),
            self.icon.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(selector_key)
        .any(|candidate| candidate == selector || candidate.ends_with(&selector))
    }

    fn is_browser(&self) -> bool {
        self.categories.as_deref().is_some_and(|categories| {
            categories
                .split(';')
                .any(|category| category == "WebBrowser")
        })
    }
}

#[derive(Debug)]
pub struct Browser {
    pub name: String,
    pub engine: Option<Engine>,
    pub color: &'static str,
    pub icon: Option<PathBuf>,
    pub version: Option<String>,
    pub profile_backend: Option<ProfileBackend>,
    pub active_profile: Option<Profile>,
    pub extensions: Vec<Extension>,
}

impl Browser {
    pub fn detect() -> Self {
        let entry = default_browser_desktop_id().and_then(|id| find_desktop_entry(&id));
        Self::from_desktop_entry(entry)
    }

    pub fn detect_selector(selector: &str) -> Option<Self> {
        if let Some(entry) = desktop_entries()
            .into_iter()
            .find(|entry| entry.matches_selector(selector))
        {
            return Some(Self::from_desktop_entry(Some(entry)));
        }

        let program = find_executable(selector)?;
        Some(Self::from_command(program))
    }

    pub fn installed_browsers() -> Vec<(String, String)> {
        let mut browsers = desktop_entries()
            .into_iter()
            .filter(DesktopEntry::is_browser)
            .map(|entry| (entry.name.unwrap_or_else(|| entry.id.clone()), entry.id))
            .collect::<Vec<_>>();
        for selector in extensions::profile_selectors() {
            let Some(program) = find_executable(&selector) else {
                continue;
            };
            let name = command_display_name(&program);
            if browsers
                .iter()
                .any(|(existing, _)| existing.eq_ignore_ascii_case(&name))
            {
                continue;
            }
            browsers.push((name, format!("PATH:{}", program.display())));
        }
        browsers.sort_by_key(|(name, id)| (name.to_ascii_lowercase(), id.to_ascii_lowercase()));
        browsers.dedup_by(|left, right| left.1 == right.1);
        browsers
    }

    pub fn completion_candidates() -> Vec<String> {
        let mut candidates = Self::installed_browsers()
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>();
        candidates.sort_by_key(|name| name.to_ascii_lowercase());
        candidates.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        candidates
    }

    fn from_desktop_entry(entry: Option<DesktopEntry>) -> Self {
        let desktop_id = entry.as_ref().map(|entry| entry.id.clone());
        let exec = entry.as_ref().and_then(|entry| entry.exec.clone());
        let icon_name = entry.as_ref().and_then(|entry| entry.icon.clone());
        let name = entry
            .as_ref()
            .and_then(|entry| entry.name.clone())
            .or_else(|| exec.as_ref().map(|spec| spec.program.clone()))
            .unwrap_or_else(|| "Unknown browser".to_string());
        Self::from_parts(desktop_id, name, exec, icon_name)
    }

    fn from_command(program: PathBuf) -> Self {
        let name = command_display_name(&program);
        let exec = CommandSpec {
            program: program.display().to_string(),
            args: Vec::new(),
            env: Vec::new(),
        };
        Self::from_parts(None, name, Some(exec), None)
    }

    fn from_parts(
        desktop_id: Option<String>,
        name: String,
        exec: Option<CommandSpec>,
        icon_name: Option<String>,
    ) -> Self {
        let profile_backend = extensions::discover_backend(exec.as_ref());
        let active_profile = profile_backend
            .as_ref()
            .and_then(ProfileBackend::active_profile);
        let extensions = profile_backend
            .as_ref()
            .map(|backend| backend.extensions(active_profile.as_ref()))
            .unwrap_or_default();
        let engine = profile_backend
            .as_ref()
            .map(ProfileBackend::engine)
            .or_else(|| {
                detect_engine_hint(
                    desktop_id.as_deref(),
                    &name,
                    exec.as_ref(),
                    icon_name.as_deref(),
                )
            });
        let color = engine.map(Engine::color).unwrap_or("117");
        let icon = icon_name.and_then(|icon| logo::resolve_icon(&icon));
        let version = exec.as_ref().and_then(browser_version);

        Self {
            name,
            engine,
            color,
            icon,
            version,
            profile_backend,
            active_profile,
            extensions,
        }
    }
}

fn default_browser_desktop_id() -> Option<String> {
    command_output("xdg-settings", &["get", "default-web-browser"])
        .filter(|value| !value.trim().is_empty())
}

fn find_desktop_entry(id: &str) -> Option<DesktopEntry> {
    desktop_entries().into_iter().find(|entry| entry.id == id)
}

fn desktop_entries() -> Vec<DesktopEntry> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();

    for directory in desktop_dirs() {
        let Ok(files) = fs::read_dir(directory) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("desktop") {
                continue;
            }
            let Some(id) = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            if !seen.insert(id.clone()) {
                continue;
            }
            let Ok(data) = fs::read_to_string(&path) else {
                continue;
            };
            entries.push(parse_desktop_entry(&id, &data));
        }
    }

    entries
}

fn parse_desktop_entry(id: &str, data: &str) -> DesktopEntry {
    DesktopEntry {
        id: id.to_string(),
        name: desktop_value(data, "Name"),
        exec: desktop_value(data, "Exec").and_then(|value| parse_exec(&value)),
        icon: desktop_value(data, "Icon"),
        categories: desktop_value(data, "Categories"),
    }
}

fn desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = home_dir() {
        dirs.push(home.join(".nix-profile/share/applications"));
        dirs.push(home.join(".local/share/applications"));
    }
    if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(data_home).join("applications"));
    }
    if let Some(profiles) = env::var_os("NIX_PROFILES") {
        dirs.extend(env::split_paths(&profiles).map(|profile| profile.join("share/applications")));
    }
    if let Some(data_dirs) = env::var_os("XDG_DATA_DIRS") {
        dirs.extend(env::split_paths(&data_dirs).map(|directory| directory.join("applications")));
    }
    dirs.push(PathBuf::from("/usr/local/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));
    dirs
}

fn find_executable(selector: &str) -> Option<PathBuf> {
    let selector_path = Path::new(selector);
    if selector_path.components().count() > 1 {
        return is_executable(selector_path).then(|| selector_path.to_path_buf());
    }

    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|directory| directory.join(selector))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        fs::metadata(path)
            .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

fn command_display_name(program: &Path) -> String {
    let raw_name = program
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("browser");
    raw_name
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn desktop_value(data: &str, key: &str) -> Option<String> {
    data.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .map(|value| value.trim().to_string())
}

fn selector_key(value: &str) -> String {
    value
        .trim()
        .strip_suffix(".desktop")
        .unwrap_or(value.trim())
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn browser_version(exec: &CommandSpec) -> Option<String> {
    command_spec_output(exec, &["--version"])
}

fn detect_engine_hint(
    desktop_id: Option<&str>,
    name: &str,
    exec: Option<&CommandSpec>,
    icon: Option<&str>,
) -> Option<Engine> {
    let probe = format!(
        "{} {} {} {}",
        desktop_id.unwrap_or_default().to_ascii_lowercase(),
        name.to_ascii_lowercase(),
        exec.map(|spec| spec.program.to_ascii_lowercase())
            .unwrap_or_default(),
        icon.unwrap_or_default().to_ascii_lowercase()
    );

    if ["epiphany", "org.gnome.epiphany", "gnome-web", "webkit"]
        .iter()
        .any(|signature| probe.contains(signature))
    {
        return Some(Engine::WebKit);
    }
    if ["servo", "servoshell"]
        .iter()
        .any(|signature| probe.contains(signature))
    {
        return Some(Engine::Servo);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(program: &str) -> CommandSpec {
        CommandSpec {
            program: program.to_string(),
            args: Vec::new(),
            env: Vec::new(),
        }
    }

    #[test]
    fn selector_matches_helium_name_and_executable() {
        let entry = parse_desktop_entry(
            "net.imput.Helium.desktop",
            "Name=Helium\nExec=/nix/store/example/bin/helium %U\nCategories=Network;WebBrowser;\n",
        );
        assert!(entry.matches_selector("helium"));
        assert!(entry.is_browser());
    }

    #[test]
    fn hints_webkit_for_gnome_web() {
        assert_eq!(
            detect_engine_hint(
                Some("org.gnome.Epiphany.desktop"),
                "Web",
                Some(&command("epiphany")),
                None
            ),
            Some(Engine::WebKit)
        );
    }

    #[test]
    fn hints_servo_without_creating_a_profile_backend() {
        assert_eq!(
            detect_engine_hint(
                Some("servo.desktop"),
                "Servo",
                Some(&command("servoshell")),
                None
            ),
            Some(Engine::Servo)
        );
    }

    #[test]
    fn leaves_unknown_engine_unknown() {
        assert_eq!(
            detect_engine_hint(
                Some("foo-browser.desktop"),
                "FooBrowser",
                Some(&command("foo-browser")),
                None
            ),
            None
        );
    }
}
