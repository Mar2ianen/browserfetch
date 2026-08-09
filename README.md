# browserfetch

A tiny, terminal-friendly browser fetcher for Linux desktops.

`browserfetch` is a deliberately small Rust CLI in the spirit of `fastfetch`,
but focused on the browser you actually use. It reads the XDG default browser's
desktop entry, shows its name, icon and version, finds extra profile data when a
known local layout is present, and prints a compact OS/session summary.

The name is a little joke. The output is useful enough to keep around.

## What it does

- detects any XDG default browser through `xdg-settings` and desktop entry
  files;
- treats Firefox and Chromium layouts as optional profile backends rather than
  hard-coded browser identities;
- reads Firefox and Chromium-family profile data from local configuration
  directories when the data format is present;
- displays installed extensions from Firefox `extensions.json` or Chromium
  `manifest.json` files;
- reports Gecko/Blink from profile backends and recognizes WebKit for GNOME Web
  and Servo for Servo-style desktop entries;
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
                                   Extensions: 12 (12 enabled, 0 disabled, 0 unknown)
```

Any browser with a valid `.desktop` entry can show its identity and icon; its
version is shown when the entry's executable can be parsed and invoked.
Profile and extension details are available when browserfetch recognizes the
local Firefox or Chromium data layout.

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

## Profile discovery

- Chromium-family profiles are discovered from `~/.config/...` by looking for
  `Local State` and profile directories.
- Firefox profiles are discovered from `~/.config/mozilla/firefox`,
  `~/.mozilla/firefox` and the common Flatpak Firefox path.
- Firefox extensions are parsed from the active profile's `extensions.json`;
  Chromium extensions are read from `manifest.json` and their state from
  `Preferences`.

Engine detection is intentionally conservative: a known profile backend or an
unambiguous desktop entry may provide an engine hint; otherwise the output
shows `unknown` instead of borrowing metadata from another installed browser.

## Development

```sh
cargo fmt --check
cargo test --locked
cargo clippy --all-targets -- -D warnings
```

Changes are checked by GitHub Actions as well.

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for the full text.
