# 響 (Hibiki)

![Hibiki Demo](./assets/showcase.gif)

A GTK4 Layer Shell keystroke visualizer for Wayland compositors, built specifically for Wayland. While tools like showmethekey exist, I've always felt they lacked a bit of that "modern" aesthetic. So, heavily inspired by the look of [KeyCastr](https://github.com/keycastr/keycastr), I decided to build my own version.

And yes, it's written in Rust, so you already know it's blazing fast and memory-safe.

## Key Features

- **Wayland Native**: No more X11 workarounds; built to work with modern compositors.
- **Two Display Modes**:
  - **Keystroke**: The classic view for showing exactly what you're hitting.
    - **Unify Words**: Optionally fold consecutive typed characters into a single bubble per word (`c`, `l`, `e`, `a`, `r` → `clear`) to cut down on clutter.
  - **Bubble**: A sleek, minimal style inspired by [devaslife's setup](https://www.youtube.com/watch?v=zu_vqAWHy_E).
- **Native Audio Engine**:
  - Built-in `rodio`-based audio engine on a dedicated thread.
  - Compatible with **Mechvibes** sound packs (single and multi-file support).
  - Zero-copy sample playback for low latency.
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

## Control from the compositor (CLI)

Hibiki ships a small CLI that talks to the running instance, so modes can be
toggled without touching the tray or settings window:

```bash
hibiki --keystroke-toggle   # switch to keystroke mode (alias: --keystroke, -k)
hibiki --bubble-toggle      # switch to bubble mode    (alias: --bubble, -b)
hibiki --toggle             # toggle between the two modes (-t)
hibiki --pause-toggle       # pause/resume capture      (-p)
hibiki --settings           # open settings
hibiki --launcher           # bring the launcher back up
hibiki --quit               # quit the running instance
```

All commands are forwarded to an already-running instance over D-Bus; if none
is running, the first command also starts Hibiki. This makes them ideal for
compositor keybindings. For example, on Hyprland:

```ini
bind = SUPER, K, exec, hibiki --keystroke-toggle
bind = SUPER, B, exec, hibiki --bubble-toggle
bind = SUPER, P, exec, hibiki --pause-toggle
```

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
