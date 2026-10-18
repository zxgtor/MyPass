# MyPass

简洁的 Windows 密码与 API Token 管理器，使用 Rust + egui，数据保存在本地 KDBX4 加密文件中。

MyPass 独立运行，无需安装或附带 KeePassXC。

界面在启动时自动跟随 Windows 用户的显示语言：中文显示简体中文，英语及其他暂不支持的语言显示英文。更改 Windows 显示语言后，重启 MyPass 生效。不会翻译或修改密码库中的账号内容。

## 开始使用

双击 `MyPass.exe`，点击「创建我的密码库」，选择保存位置，设置并确认至少 12 个字符的主密码。
主密码无法重置或找回，请妥善记住，并单独备份 `.kdbx` 文件。

解锁后点击「添加」，选择「账号密码」或「API Token」。填写名称与秘密值，保存后即可在列表里搜索和复制。点击名称查看或编辑，更多菜单提供复制用户名、打开网站和删除。

- 保存自动完成；加密处理时会显示等待状态，失败时保留编辑表单。
- 5 分钟无操作自动锁定，也可点击「锁定」或按 Ctrl+L。
- 复制后 30 秒清除剪贴板；若剪贴板已换成其他内容，则不清除。退出与锁定时也会尝试清除。
- 每次修改前，原文件保存为同目录的 `.kdbx.bak`；只保留上一版。这不能替代独立备份。
- 删除会移除当前条目，旧备份和条目历史可能仍保留过去的内容。
- 可打开仅使用主密码的 KDBX4.0 / 4.1 文件，保存时使用 KDBX4.1。保留其他字段和分组，列表隐藏回收站。
- 默认不联网。仅手动选择「打开网站」时启动浏览器，不会把密码或 Token 加入网址。

## 当前范围与限制

这是首版，尚未经过独立安全审计。`keepass` 库的 KDBX 写入支持被上游标为实验性；已用本机 KeePassXC 2.7.12 CLI 验证读取、修改和再次保存。重要的现有密码库请先使用副本。

首版不支持密钥文件、YubiKey、浏览器自动填充、云同步、主密码修改及多文件自动合并。请避免同时在其他程序修改同一个文件；检测到文件内容改变会拒绝保存。

剪贴板清理无法删除 Windows 剪贴板历史或其他软件已复制走的内容。解锁期间，应用必须在内存中持有解密的数据。

程序只记住最近密码库路径，存于 `%LOCALAPPDATA%\MyPass\last-vault.txt`，不保存主密码。

## 构建与验证

需要 Windows Rust MSVC 工具链与 Visual Studio C++ Build Tools。

```powershell
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
```

发布程序位于 `target\release\mypass.exe`。界面使用 Windows 自带的微软雅黑显示中文。

KeePassXC 互操作测试默认忽略。如需重跑，设置 `KEEPASSXC_CLI` 为外部 `keepassxc-cli.exe` 的完整路径，然后运行 `cargo test keepassxc_reads_and_edits_our_database -- --ignored`。

开发版可用 `cargo run -- --preview` 查看仅含虚构数据的界面；附加 `--editor` 显示 Token 表单。预览不能保存；发布版不启用该入口。
