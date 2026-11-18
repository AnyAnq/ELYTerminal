# Development

English / [中文](../zh_cn/develop.md)

Each platform's script builds its release in `target/release`: on macOS, `scripts/mac-app.sh` builds `ELYTerminal.app`, with its icon, and a disk image of it; on Windows, `scripts/windows-zip.ps1` zips `elyterminal.exe`, its icon inside, with Microsoft's `conpty.dll`, `OpenConsole.exe`, `OpenConsoleProxy.dll` and their license; on Linux, `scripts/linux-packages.sh` builds once in Docker on Ubuntu 22.04 and packs the same binary as tar.gz, DEB, AppImage and RPM. `scripts/linux-tar.sh` still builds the tarball on its own. [Linux packaging](../../packaging/linux/README.md) covers individual scripts, validation and release uploads.

On Linux, install the libraries gpui links first. On Debian and Ubuntu:

```sh
sudo apt-get install pkg-config libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libxcb1-dev libx11-xcb-dev libfontconfig-dev libfreetype-dev
```

The app writes its log to stderr, its own info lines and Ely's. `RUST_LOG=elyterminal=debug,ely_gpui_component=info` writes more; `trace` logs each paint. A Windows release build opens no console: `elyterminal.exe 2> log.txt` keeps its log.

`scripts/check.sh` runs fmt, clippy, the tests and the 500-line rule. [AGENTS.md](../../AGENTS.md) lists the rest: the layout, the decisions and the tools.

On Windows with a desktop and GPU, `cargo test --lib native_assistant_repairs_a_project_and_manual_input_stops_it -- --ignored --nocapture --test-threads=1` checks the AI panel against a local fake service and a real ConPTY shell, without using your model settings. It creates an isolated project and screenshots under `target/ai-native-*`. `ELY_AI_TEST_SHELL` selects `powershell.exe` (default), `pwsh.exe` or `cmd.exe`; no external provider is contacted.

The fixture omits the optional `run_command.requires_approval` hint for safe, declined and automatically approved commands. The application still applies its command policy; command/reason fields, field types and unknown arguments remain validated.

Set `ELY_AI_BENCH=1` for the native test to measure long-reply frame CPU time, readiness checks, AI settings opening, the model dropdown and selection saves. These desktop regression budgets include Windows window-creation overhead; the configuration remains isolated.
