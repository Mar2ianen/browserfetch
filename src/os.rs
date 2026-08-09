use std::{env, fs};

use crate::util::command_output;

pub fn summary() -> String {
    let pretty = fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|data| {
            data.lines()
                .find_map(|line| line.strip_prefix("PRETTY_NAME="))
                .map(|value| value.trim_matches('"').to_string())
        })
        .unwrap_or_else(|| env::consts::OS.to_string());
    let kernel = command_output("uname", &["-r"]).unwrap_or_else(|| "unknown kernel".to_string());
    format!("{pretty} / {kernel}")
}

pub fn session_summary() -> String {
    let session_type = env::var("XDG_SESSION_TYPE")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            env::var("WAYLAND_DISPLAY")
                .ok()
                .filter(|value| !value.is_empty())
                .map(|_| "wayland".to_string())
        })
        .or_else(|| {
            env::var("DISPLAY")
                .ok()
                .filter(|value| !value.is_empty())
                .map(|_| "x11".to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    let desktop = env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            env::var("DESKTOP_SESSION")
                .ok()
                .filter(|value| !value.is_empty())
        })
        .or_else(detect_wm_process)
        .unwrap_or_else(|| "unknown".to_string());

    format!("{session_type} / {desktop}")
}

fn detect_wm_process() -> Option<String> {
    let output = command_output("ps", &["-e", "-o", "comm="])?;
    let processes = output.to_lowercase();
    [
        "niri",
        "hyprland",
        "sway",
        "river",
        "wayfire",
        "kwin_wayland",
        "kwin_x11",
        "gnome-shell",
        "mutter",
        "cinnamon",
        "xfwm4",
        "openbox",
        "i3",
        "awesome",
        "bspwm",
        "dwm",
    ]
    .into_iter()
    .find(|name| processes.contains(name))
    .map(str::to_string)
}
