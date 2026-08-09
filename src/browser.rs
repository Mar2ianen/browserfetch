use std::fmt;
use std::fs;
use std::path::PathBuf;

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
        let desktop_id = default_browser_desktop_id();
        let desktop = desktop_id.as_deref().and_then(read_desktop_file);
        let exec = desktop
            .as_ref()
            .and_then(|data| desktop_value(data, "Exec"))
            .and_then(|value| parse_exec(&value));
        let icon_name = desktop
            .as_ref()
            .and_then(|data| desktop_value(data, "Icon"));
        let name = desktop
            .as_ref()
            .and_then(|data| desktop_value(data, "Name"))
            .or_else(|| exec.as_ref().map(|spec| spec.program.clone()))
            .unwrap_or_else(|| "Unknown browser".to_string());

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
