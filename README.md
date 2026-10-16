# 響 (Hibiki)

![Hibiki Demo](./assets/showcase.gif)

A GTK4 Layer Shell keystroke visualizer for Wayland compositors, built specifically for Wayland. While tools like showmethekey exist, I've always felt they lacked a bit of that "modern" aesthetic. So, heavily inspired by the look of [KeyCastr](https://github.com/keycastr/keycastr), I decided to build my own version.

And yes, it's written in Rust, so you already know it's blazing fast and memory-safe.

## Key Features

- **Wayland Native**: No more X11 workarounds; built to work with modern compositors.
- **Two Display Modes**:
  - **Keystroke**: The classic view for showing exactly what you're hitting.
    - **Unify Words**: Optionally fold consecutive typed characters into a single bubble per word (`c`, `l`, `e`, `a`, `r` → `clear`) to cut down on clutter. Backspace is drawn as its own key and starts a new word instead of erasing the grouped text; set `backspace_edits_word = true` in the config file to restore word editing.
  - **Bubble**: A sleek, minimal style inspired by [devaslife's setup](https://www.youtube.com/watch?v=zu_vqAWHy_E).
- **Native Audio Engine**:
  - Built-in `rodio`-based audio engine on a dedicated thread.
  - Compatible with **Mechvibes** sound packs (single and multi-file support).
  - Zero-copy sample playback for low latency.
  - **Sounds** are a standalone feature: enable them alongside Keystroke
    or Bubble mode, or turn off the visualizer to *only* hear the keystrokes.
    Configured once under **Settings → Sounds**.
- **Modern Configuration**:
  - Configuration migrated to `config.toml` (auto-migrates from old JSON).
  - **Granular Typography**: Customize font family, weight, and size per mode.
  - **Custom Colors**: Theme the background and text of keys and bubbles.
  - **Virtualization**: Optimized Settings UI handles large lists of fonts and sound packs with zero lag.
- **System Integration**:
  - **GTK Theme Support**: Automatically respects your system theme.
  - **System Tray**: Quick access to toggle modes, pause capture, or open settings.
  - **CLI Control**: Toggle modes from compositor keybindings (`hibiki --keystroke-toggle`).
  - **Draggable**: Visualizer windows can be repositioned easily.

## Supported Compositors

Hibiki attempts to automatically detect your running compositor.

| Compositor    | Support Level           | Notes                                                  |
| ------------- | ----------------------- | ------------------------------------------------------ |
| **Niri**      | 🟢 **Tested & Working** | Fully supported layout detection and event handling.   |
| **Umbriel**   | 🟡 **Experimental**     | IPC-based layout detection and event handling.         |
| **Hyprland**  | 🟡 **Experimental**     | Implemented but needs community testing.               |
| **Sway**      | 🟡 **Experimental**     | Implemented but needs community testing.               |
| **River/DWL** | 🔴 **Basic/None**       | Layout detection may fail; "Basic support" is minimal. |

> **Note**: Real compositor support is complex! I can't detect layouts reliably on all compositors without specific implementations. **I need help testing Hyprland and Sway!** If you use these, please report issues.

## Installation

### Prerequisites

You need a Wayland compositor and the following system libraries:

- `libgtk-4-dev`
- `libgtk4-layer-shell-dev`
- `libasound2-dev` (for audio)

### Via Cargo

If you have Rust installed:

```bash
cargo install --path .
# or
cargo run --release
```

### Via the install script (prebuilt binary)

The fastest way on any Linux system:

```bash
curl -fsSL https://raw.githubusercontent.com/linuxmobile/hibiki/main/install.sh | bash
```

This downloads the prebuilt binary for your architecture and installs it to
`~/.local/bin` together with a desktop entry and icon. Useful options:

```bash
# install system-wide to /usr/local (uses sudo)
curl -fsSL .../install.sh | bash -s -- --system

# pin a specific release
curl -fsSL .../install.sh | bash -s -- --version v0.2.0

# remove it again
curl -fsSL .../install.sh | bash -s -- --uninstall
```

The script detects your package manager (pacman/apt/dnf) and tells you which
runtime libraries are missing, and it checks that your glibc is new enough for
the binary.

> **Debian / Ubuntu:** the prebuilt binary needs a system
> `gtk4-layer-shell`, which is only packaged on Ubuntu 25.10+ and Debian 13+.
> On older releases (Ubuntu 24.04, Debian 12) use the **AppImage** below —
> it bundles GTK, gtk4-layer-shell and everything else.

### Via AppImage

A self-contained AppImage is attached to every release. It needs no GTK or
gtk4-layer-shell from the system, so it works on any distro with a recent
glibc:

```bash
curl -fsSL https://raw.githubusercontent.com/linuxmobile/hibiki/main/install.sh | bash -s -- --appimage
```

Or grab `hibiki-<version>-<arch>.AppImage` from the
[releases page](https://github.com/linuxmobile/hibiki/releases), then:

```bash
chmod +x hibiki-*-x86_64.AppImage
./hibiki-*-x86_64.AppImage
```

If your system has no FUSE, run it with `--appimage-extract-and-run`.

After installing either way, add your user to the `input` group so Hibiki can
read the keyboard devices (then log out and back in):

```bash
sudo usermod -aG input "$USER"
```

### Arch Linux

Two mutually-exclusive `PKGBUILDs` are provided under `packaging/arch/` — install
only one (`hibiki-bin` declares `conflicts=('hibiki')`).

**From source (`hibiki`)** — compiles from the release tarball:

```bash
git clone https://github.com/linuxmobile/hibiki
cd hibiki/packaging/arch
makepkg -si
```

**Prebuilt binary (`hibiki-bin`)** — downloads the matching release binary
instead of compiling, so it installs in seconds:

```bash
git clone https://github.com/linuxmobile/hibiki
cd hibiki/packaging/arch/hibiki-bin
makepkg -si
```

Both need the `base-devel` group (`sudo pacman -S --needed base-devel`). With an
AUR helper, point it at the directory containing the `PKGBUILD` you want.

When a new version is released, bump `pkgver` and run `updpkgsums` in the
relevant directory before building. An AUR package is on the way.

### Via Nix

Flake support is included!

```bash
# Run directly
nix run github:linuxmobile/hibiki

# Build
nix build github:linuxmobile/hibiki

# Develop
nix develop
```

## Configuration

Hibiki reads its configuration from `~/.config/hibiki/config.toml`. Beyond
typography and behavior, every mode can be themed with custom colors:

```toml
# Keystroke mode
background = "#1e1e2e"   # key bubble background (omit for theme default)
foreground = "#cdd6f4"   # key label color (omit for theme default)

[bubble]
background = "#181825"   # bubble background
foreground = "#cdd6f4"   # bubble text color
```

Colors accept hex (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`) or
`rgb()`/`rgba()` notation. When `background`/`foreground` are omitted, the
active GTK theme is used, so light/dark switching keeps working. The same
options are available in the **Settings → Colors** section of the UI.

### Super key icon

The Super/Meta key can show Hibiki's built-in logo (default), an icon from your
current icon theme, or a custom image:

```toml
[super_key_icon]
# "hibiki" (default) | "symbolic" | "icon" | "image"
mode = "symbolic"
# Icon theme name for the "symbolic" and "icon" modes.
icon_name = "start-here-symbolic"
# Path to a PNG or WebP file for the "image" mode.
# image_path = "/home/you/.config/hibiki/logo.png"
```

`symbolic` uses a monochrome icon tinted like the key text, while `icon` uses
the theme's full-color artwork. `image` loads a PNG or WebP that **must have a
transparent background** (an alpha channel); files without transparency are
ignored and the built-in logo is used instead. Everything is configurable from
**Settings → Keystrokes → Super Key Icon**, which offers a searchable icon
picker with a live preview.

### Sounds

Key sounds are configured once and shared by every mode. They used to
live under `[audio]` / `[bubble.audio]`; existing configs are migrated
automatically on load.

```toml
[sound]
enabled = true                  # play sounds on every keypress
volume = 0.8                    # 0.0 – 1.0
sound_pack = "cherrymx-blue-abs"

# Extra folder to scan for your own packs (optional).
# folder = "/home/you/Downloads/mechvibes-packs"

# Draw the keystroke/bubble overlay. Set to false for sound only.
show_visual = true
```

Some sounds are **built into the binary**, so they are available no
matter how Hibiki was installed (`cargo install`, Nix, or a plain binary):

- CherryMX Blue (ABS) — `cherrymx-blue-abs`
- CherryMX Brown (ABS) — `cherrymx-brown-abs`
- CherryMX Red (ABS) — `cherrymx-red-abs`
- EG Oreo — `eg-oreo`
- NK Cream — `nk-cream`

To add more, drop Mechvibes-format pack folders into
`~/.local/share/hibiki/sounds`, or use **Settings → Sounds → Choose folder…** to
load packs from any other directory. A pack is a folder containing a
`config.json` plus its audio files (`wav`/`ogg`); the info button in that tab
has the details. Packs found in a folder override built-ins with the same name.

Flip sound and visual on/off at runtime with `hibiki --sound-toggle` and
`hibiki --visual-toggle`, or from the **Settings → Sounds** tab.

## Control from the compositor (CLI)

Hibiki ships a small CLI that talks to the running instance, so modes can be
toggled without touching the tray or settings window:

```bash
hibiki --keystroke-toggle   # switch to keystroke mode (alias: --keystroke, -k)
hibiki --bubble-toggle      # switch to bubble mode    (alias: --bubble, -b)
hibiki --toggle             # toggle between the two modes (-t)
hibiki --pause-toggle       # pause/resume capture      (-p)
hibiki --sound-toggle       # enable/disable sounds
hibiki --visual-toggle      # show/hide the visualizer (sound only)
hibiki --settings           # open settings
hibiki --launcher           # bring the launcher back up
hibiki --quit               # quit the running instance
hibiki --daemon             # start detached in the background (-d)
```

All commands are forwarded to an already-running instance over D-Bus; if none
is running, the first command also starts Hibiki. This makes them ideal for
compositor keybindings. The commands are silent by default; add `--verbose`
(`-v`) or set `RUST_LOG` to see logs. For example, on Hyprland:

```ini
bind = SUPER, K, exec, hibiki --keystroke-toggle
bind = SUPER, B, exec, hibiki --bubble-toggle
bind = SUPER, P, exec, hibiki --pause-toggle
```

### In-app global hotkeys

Hibiki also listens for its own global hotkeys directly on the keyboard
devices, so they work on any compositor (KDE, GNOME, Hyprland, Niri, Umbriel,
Sway, …) without extra configuration. They are captured in **Settings**, and
the current bindings are always shown in the launcher:

| Action                        | Default              |
| ----------------------------- | -------------------- |
| Toggle keystroke / bubble     | `Ctrl + Shift + T`   |
| Toggle focus (Bubble mode)    | `Ctrl + B`           |
| Lose focus (Bubble mode)      | `Esc`                |
| Pause / resume capture        | `Ctrl + P`           |
| Switch to keystroke mode      | `Ctrl + Shift + K`   |
| Switch to bubble mode         | `Ctrl + Shift + B`   |

#### Avoiding conflicts with other applications

Because Hibiki reads the keyboard directly, a combo it handles (for example
`Ctrl + Shift + B`) is *also* still delivered to the focused application, which
can trigger the browser's own shortcuts.

To avoid this, Hibiki can register its hotkeys through the XDG Desktop Portal
`GlobalShortcuts` interface. The desktop then owns the combos, does not forward
them to the focused app, and shows them in its own shortcut settings so they can
be rebound. Pick the behaviour under **Settings → General → Shortcut Handling**:

| Mode                 | Behaviour                                                        |
| -------------------- | ---------------------------------------------------------------- |
| **Automatic** (default) | Use the portal when the desktop supports it, otherwise fall back to direct device capture. |
| **Desktop portal**   | Only use the portal (Hotkeys are inactive if it is unavailable). |
| **Direct (device)**  | Only use direct evdev capture, ignoring the portal.              |

The portal is available on desktops that implement it (for example recent
GNOME and KDE Plasma); everywhere else Hibiki keeps working via direct capture.
Changing the mode takes effect after a restart.

## Contributing & Help Needed

The project is currently in **Early WIP**.

### 🛑 We Need Your Help!

1.  **Compositor Testing**: We specifically need testers for **Hyprland** and **Sway**. The code is there, but I only use Niri.
2.  **Packaging**: We need maintainers for **AUR**, **Debian/Ubuntu**, **Fedora**, etc.
3.  **Feedback**: Found a bug? Open an issue!

### Development

1.  Clone the repo.
2.  Run `cargo run` to start dev mode.
3.  Check `src/compositor` to understand how we handle different Wayland protocols.

---

If you find this useful, I'd really appreciate it if you could:

- ⭐ Drop a star on the repo!
- 💡 Open an Issue with ideas.
- 🛠️ Submit a PR.
