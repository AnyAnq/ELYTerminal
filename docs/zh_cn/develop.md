# 开发

[English](../en_us/develop.md) / 中文

各平台的脚本把发行构建写到 `target/release`：在 macOS 上，`scripts/mac-app.sh` 构建带图标的 `ELYTerminal.app` 及其磁盘映像；在 Windows 上，`scripts/windows-zip.ps1` 把内含图标的 `elyterminal.exe` 连同 Microsoft 的 `conpty.dll`、`OpenConsole.exe`、`OpenConsoleProxy.dll` 及其许可打成 zip；在 Linux 上，`scripts/linux-packages.sh` 在 Ubuntu 22.04 的 Docker 里构建一次，再把同一份二进制打成 tar.gz、DEB、AppImage 和 RPM。`scripts/linux-tar.sh` 仍可单独构建压缩包。[Linux 打包](../../packaging/linux/README.md) 说明各脚本、校验和上传发行。

在 Linux 上，先安装 gpui 链接的库。Debian 和 Ubuntu：

```sh
sudo apt-get install pkg-config libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libxcb1-dev libx11-xcb-dev libfontconfig-dev libfreetype-dev
```

应用把日志写到 stderr，自己的 info 行和 Ely 的。`RUST_LOG=elyterminal=debug,ely_gpui_component=info` 写得更多；`trace` 记录每一次绘制。Windows 的发行构建不打开控制台：`elyterminal.exe 2> log.txt` 留下日志。

`scripts/check.sh` 跑 fmt、clippy、测试和 500 行规则。其余在 [AGENTS.md](../../AGENTS.md)：布局、决定和工具。

Windows 上有桌面和 GPU 时，运行 `cargo test --lib native_assistant_repairs_a_project_and_manual_input_stops_it -- --ignored --nocapture --test-threads=1`，可通过本地模拟服务和真实 ConPTY shell 检查 AI 面板，不使用你的模型设置。它在 `target/ai-native-*` 下创建隔离项目并保存截图。环境变量 `ELY_AI_TEST_SHELL` 可选 `powershell.exe`（默认）、`pwsh.exe` 或 `cmd.exe`；不会连接外部模型服务。

原生模拟模型会在安全命令、拒绝执行和自动允许场景中省略可选的 `run_command.requires_approval` 提示。应用仍独立应用命令审批策略；命令、原因、字段类型和未知参数的校验会保留。

设置环境变量 `ELY_AI_BENCH=1` 后运行上述原生测试，会额外测量长回复帧耗时、终端就绪检查、打开 AI 设置、模型下拉和选择保存。性能阈值用于本机桌面回归，包含 Windows 新建窗口的系统开销；测试仍使用隔离配置。
