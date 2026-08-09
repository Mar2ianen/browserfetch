<div align="center">

# browserfetch

### `fastfetch`, but your browser has been selected for inspection.

**Linux · Rust · XDG · browser archaeology**

> browserfetch does not support *Helium*.
> browserfetch supports **browsers**.

</div>

---

`browserfetch` is a small Linux CLI that asks your desktop what browser you use,
finds the browser's real icon, pokes at whatever local profile data can be
identified without lying, and prints the result in a fetch-style terminal view.

It started as a joke.

Unfortunately, it now has architecture.

```text
                 xdg-settings / selector
                          │
                          ▼
                    .desktop entry
                 Name / Exec / Icon
                          │
            ┌─────────────┴─────────────┐
            │                           │
            ▼                           ▼
      generic identity           optional evidence
      version + logo         profile format / self-report
            │                           │
            │               ┌───────────┼────────────┐
            │               ▼           ▼            ▼
            │            Firefox     Chromium     Epiphany
            │               │           │            │
            │             Gecko       Blink      WebKitGTK
            │
            └───────────────────────────────► terminal

                          Servo
                            │
                      `--version`
                            │
                            ▼
                         Servo
```

No giant hard-coded browser Pokédex is required.

## What the cursed thing does

- detects the default XDG browser through `xdg-settings`;
- lets you select another installed browser by display name, desktop ID or
  executable;
- discovers `.desktop` files from normal XDG locations and common Nix profile
  locations;
- renders the browser's actual desktop icon through `chafa`;
- reads the browser version from its configured command;
- recognizes Firefox, Chromium and Epiphany **profile formats** as optional
  backends instead of treating browser brands as types;
- derives Gecko, Blink and WebKitGTK from those profile backends;
- recognizes Servo from Servo's own version output;
- reads Firefox extensions from `extensions.json`;
- reads Chromium-family extensions from on-disk manifests **and** embedded
  `Preferences` metadata;
- resolves Chromium `__MSG_name__` extension names through `_locales`;
- shows enabled / disabled / unknown extension state without pretending
  `unknown == enabled`;
- handles Unicode terminal width properly;
- has dynamic completion for zsh, bash and fish;
- performs no network requests and uploads absolutely nothing.

## Exhibit A

```text
chechulin@archlinux
----------------------------------
Browser    : Helium
Engine     : Blink
Version    : Helium 0.15.2.1 (Chromium ...)
OS         : Arch Linux / 6.x
Session    : wayland / niri
Profile    : Default / Default
Extensions : 3 (3 enabled, 0 disabled, 0 unknown)
Enabled    :
           : ├─ Chromium PDF Viewer 1
           : ├─ extstore-fixups 0.0.0
           : └─ uBlock Origin 1.72.2
```

Helium is not special-cased here. Its desktop entry provides its identity and
icon; its local data proves that the profile is Chromium-family; the engine
follows from that evidence.

That distinction is the entire point.

## Browser identity is not browser family

A browser does **not** have to be known to browserfetch to be displayed.

If Linux has a valid desktop entry for `SuperMegaFoxFork3000`, browserfetch can
already use its `Name`, `Exec` and `Icon`. Extra information appears only when
there is evidence for it.

Current profile backends:

| Evidence | Engine | Profile | Extensions |
|---|---|---:|---:|
| Firefox-family profile layout | Gecko | yes | yes |
| Chromium-family profile layout | Blink | yes | yes |
| Epiphany profile layout | WebKitGTK | backend root | no |
| Servo self-reported version | Servo | no | no |
| Unknown browser | unknown | unknown | no |

`unknown` is a valid answer. Inventing metadata is not.

## Usage

Use the default browser:

```sh
browserfetch
```

Inspect a specific browser:

```sh
browserfetch Firefox
browserfetch Google Chrome
browserfetch Helium
browserfetch org.gnome.Epiphany.desktop
```

Names with spaces may be quoted or passed as separate words.

List discovered browsers:

```sh
browserfetch --list
```

Internal completion source:

```sh
browserfetch --complete
```

Help:

```sh
browserfetch --help
```

## Tab completion, because typing is a design failure

Completion candidates are generated dynamically by `browserfetch --complete`,
so installing another browser does not require editing a completion file.

### zsh

```sh
fpath=(/path/to/browserfetch/completions $fpath)
autoload -Uz compinit && compinit
```

### bash

```sh
source /path/to/browserfetch/completions/browserfetch.bash
```

### fish

```fish
source /path/to/browserfetch/completions/browserfetch.fish
```

Then:

```text
$ browserfetch <TAB>
Firefox    Google Chrome    Helium    ...whatever you installed at 03:17
```

## Profile discovery

### Firefox-family

browserfetch looks for real Firefox-style profile roots and validates the
layout through `profiles.ini` / profile data before using it. The active profile
and extension state are read from local Firefox metadata.

### Chromium-family

Chromium-style roots are recognized through `Local State` and profile data.
`profile.last_used` is used when available instead of blindly assuming
`Default`.

Extension metadata may come from:

```text
Profile/Extensions/<id>/<version>/manifest.json
```

or from:

```text
Preferences → extensions.settings
```

This matters for browsers that package built-in or managed extensions in ways
that do not produce a normal `Extensions/<id>` tree.

### Epiphany

Epiphany is treated as its own profile backend. browserfetch looks under XDG
data locations and the Flatpak data path and requires structural profile markers
before reporting WebKitGTK.

### Servo

Servo does not need a fake profile backend. If the executable identifies itself
as Servo through its version output, browserfetch reports `Engine: Servo`.

Beautifully stupid. Semantically correct.

## Desktop and Nix discovery

Desktop entries are searched in normal user/system XDG application directories,
plus common Nix locations such as:

```text
~/.nix-profile/share/applications
$NIX_PROFILES/*/share/applications
$XDG_DATA_HOME/applications
$XDG_DATA_DIRS/*/applications
```

So an application can be found even when its package name, desktop name and
actual executable name have decided to become three unrelated concepts.

## Logo rendering

If `chafa` is installed, browserfetch resolves the icon named by the desktop
entry and renders it as terminal symbols.

```text
.desktop → Icon=whatever → icon theme → chafa → pixels, but cursed
```

`chafa` is forced into symbol output so terminal graphics protocols do not wreck
layout calculations in terminals such as foot or Ghostty.

If no usable icon or `chafa` is available, browserfetch falls back to a text
logo rather than exploding dramatically.

## Install

Requirements:

- Linux;
- Rust 1.85+;
- `chafa` for image-based logos (optional).

Build:

```sh
cargo build --release
```

Run directly:

```sh
./target/release/browserfetch
```

Or install from the checkout:

```sh
cargo install --path .
```

## Dependencies

The dependency graph has not yet achieved sentience:

```toml
serde_json = "1"
unicode-width = "0.2.2"
```

`unicode-width` exists because reimplementing Unicode terminal-cell width by
hand was, briefly, considered a reasonable life choice.

## Development

```sh
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

GitHub Actions runs the same general quality checks.

The preferred behavior is conservative:

```text
know it  → print it
prove it → infer it
guess it → absolutely not
```

## Why?

Because the world already had fetch programs for the OS, hardware, Git repos and
probably several household appliances.

The browser was getting suspiciously comfortable.

## License

MIT. See [LICENSE](LICENSE).
