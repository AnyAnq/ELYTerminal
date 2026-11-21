# ELYTerminal

[![Fork release](https://img.shields.io/github/v/release/AnyAnq/ELYTerminal?style=flat-square&color=24292f)](https://github.com/AnyAnq/ELYTerminal/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-3da639?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.95-dea584?style=flat-square)](https://www.rust-lang.org)
[![GPUI](https://img.shields.io/badge/GPUI-0.3-5b4cff?style=flat-square)](https://crates.io/crates/gpui-pre)
[![Ely](https://img.shields.io/badge/Ely-components-6d28d9?style=flat-square)](https://github.com/ELY-Open-Source/Ely-GPUI-Components)
[![alacritty](https://img.shields.io/badge/alacritty-0.26-f46d01?style=flat-square)](https://github.com/alacritty/alacritty)

English / [中文](README_zh.md)

A terminal for macOS, Windows and Linux, in Rust on GPUI and Ely components. Sessions sit on the left, the terminal in the middle, Git or Files on the right. Every setting has a control in the settings window.

## Install

**This fork (Windows x64):** Download the installer or portable ZIP from the [latest fork release](https://github.com/AnyAnq/ELYTerminal/releases/latest). On Windows 10 or later, run the `-windows-x86_64-setup.exe` installer, or extract the `-windows-x86_64.zip` archive and run `elyterminal.exe` with the other extracted files kept beside it. See the [Windows install notes](docs/en_us/install.md#windows) for SmartScreen and Defender.

**Upstream builds:** For Windows ARM64, macOS (Apple silicon), or Linux x64, see the [upstream releases](https://github.com/ELY-Open-Source/ELYTerminal/releases/latest). The following package-manager commands also install the **upstream** version, not this fork:

- Homebrew (macOS): `brew install --cask ely-open-source/tap/elyterminal`
- winget (Windows, if listed in the catalog): `winget install ELYOpenSource.ELYTerminal`

## What it does

- Sessions sit on the left, the terminal in the middle, Git or Files on the right.
- Shell integration for zsh, bash, fish and PowerShell, so Files and Git follow the session's folder.
- A session splits into panes, right or down, each its own terminal; a drag resizes them and a key swaps them.
- Sessions gather in named groups and open again at the next start.
- Profiles beside +: your own command lines, and the shells this system holds, WSL distributions and Git Bash among them.
- Ely peeks over the window's edge and follows the session you are in.
- Every shortcut can be changed, and `elyterminal ctl` lets scripts and agents list, read and type into the running sessions, once Settings turns it on.
- Images through kitty's graphics protocol, animations too, and the clipboard through kitty's OSC 5522, which asks you before a program reads what it holds, unless you allowed that program for the session or it answers a paste.
- Background opacity from 0% and blur on macOS, Windows, Wayland and X11, and Mica on Windows 11; a background picture under the terminal or the whole window, sized, placed and tiled as CSS does, with a gradient over it, its own opacity and a mask.

The rest is in [docs/en_us/features.md](docs/en_us/features.md).

## Keys

Settings › Terminal lists them all, as this platform binds them. The table is in [docs/en_us/keys.md](docs/en_us/keys.md).

## Settings

The settings window saves each change at once to `settings.json` in the platform's config folder:

- macOS: `~/Library/Application Support/ELYTerminal/settings.json`
- Linux: `$XDG_CONFIG_HOME/ELYTerminal/settings.json`, else `~/.config/ELYTerminal/settings.json`
- Windows: `%APPDATA%\ELYTerminal\settings.json`

A file that does not parse, or holds a value out of range, stops the start with an error naming the file and the field. The app never overwrites it.

Settings › Appearance › Language shows the interface in English or 简体中文.

Settings › Terminal checks for a newer release at start and once a day, or at once with Check now; a newer one shows Update, which downloads and installs it in the app, its SHA-256 checked, then Restart. The check and the download run `curl`, which macOS and Windows ship.

## Build

Rust 1.95.0, which `rust-toolchain.toml` pins; rustup installs it on the first build.

```sh
cargo run --release
```

Release scripts, Linux libraries and the log are in [docs/en_us/develop.md](docs/en_us/develop.md).

## License

MIT, in `LICENSE`. Box drawing and key encoding follow Ghostty's code, MIT, in `LICENSE-GHOSTTY`. The theme files come from iTerm2-Color-Schemes, MIT, with its license in `assets/themes`. JetBrains Mono and its Nerd Font build are under the SIL Open Font License, in `assets/fonts`. The compound text decoder's tables come from libX11, under SunSoft's and Bruno Haible's notice, in `vendor/xim-ctext/tables/LICENSE`. Microsoft's ConPTY, `conpty.dll` and `OpenConsole.exe`, is MIT, in `vendor/conpty/LICENSE`.
