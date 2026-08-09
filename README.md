# browserfetch

A tiny, terminal-friendly browser fetcher for Linux desktops.

`browserfetch` is a deliberately small Rust CLI in the spirit of `fastfetch`,
but focused on the browser you actually use. It detects the XDG default browser,
shows its engine and version, finds the active profile, lists extensions, and
prints a compact OS/session summary.

The name is a little joke. The output is useful enough to keep around.

## What it does

- detects the default browser through `xdg-settings` and desktop entry files;
- recognizes Firefox, Chromium, Chrome, Brave, Vivaldi, Edge, Opera and GNOME
  Web/WebKit-style desktop entries;
- reads Firefox and Chromium-family profile data from local configuration
  directories;
- displays installed extensions from Firefox `extensions.json` or Chromium
  `manifest.json` files;
- renders the browser icon with optional `chafa`, falling back to a text box;
- stays local: it does not make network requests or upload browser data.

## Example

```text
user@host
----------------------------------  Browser   : Brave Browser
                                   Engine    : Blink
                                   Version   : 1.2.3
                                   OS        : Linux / 6.x
                                   Session   : wayland / niri
                                   Profile   : Default / Default
                                   Extensions: 12 (12 enabled, 0 disabled)
```

The exact output depends on the desktop entry and browser profile installed on
the machine.

## Install

```sh
cargo install --path .
```

After that the command is available as:

```sh
browserfetch
```

## Requirements

- Linux with an XDG desktop environment;
- Rust 1.85 or newer (the project uses edition 2024);
- `chafa` is optional and only needed for image-based logos.

## Build

```sh
cargo build --release
```

## Run

```sh
cargo run
```

or:

```sh
./target/release/browserfetch
```

## Supported profile locations

- Chromium-family profiles are read from `~/.config/...`.
- Firefox profiles are read from `~/.config/mozilla/firefox`,
  `~/.mozilla/firefox` and the common Flatpak Firefox path.
- Firefox extensions are parsed from the active profile's `extensions.json`.

## Development

```sh
cargo fmt --check
cargo test --locked
cargo clippy --all-targets -- -D warnings
```

Changes are checked by GitHub Actions as well.

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for the full text.
