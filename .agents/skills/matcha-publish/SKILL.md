---
name: matcha-publish
version: 1.0.0
description: 发布 Matcha 发行包到 GitHub Releases。版本号记录在 config.json（默认 0.1.0），每成功发布一次自动把最后一位 +1 并同步 workspace 版本。当用户说发布/发版 matcha、publish/release matcha、上传 dist 产物或安装包时使用。
---

# Matcha 发布

> **触发前提（硬性规则）**：只有当用户在请求中**明确提及 GitHub / GitHub Releases / 远程发布**时，才允许执行本脚本的远程发布。用户只说"发布""发版""打个包""本地发布"等，一律只做本地构建打包（`cargo xtask dist`），产物在 `target/dist/` 下供本地测试，**绝不**推 tag、不创建 GitHub release。

把 `target/dist/matcha-<version>-windows-x86_64/` 下的产物（`matcha.exe`、`matcha.com`、`matcha-comp.exe`、`README.txt`）连同同名 zip 发布到 GitHub Releases（默认仓库 `slb1988/Matcha`，在 config.json 中可改）。

## 版本号规则

- 权威来源：本目录 `config.json` 的 `version` 字段，含义是"**下一次要发布的版本**"，初始 0.1.0。
- 每成功发布一次，`publish.ps1` 自动把最后一位 +1（0.1.0 → 0.1.1 → …）。
- 同时同步根 `Cargo.toml` 的 `[workspace.package] version`，保证下一次 `cargo xtask dist` 生成的目录名（`matcha-<version>-windows-x86_64`）与配置一致。`Cargo.lock` 会在下次 cargo 构建时自动刷新。
- 用户要跳到指定版本：直接改 `config.json` 再执行发布。

## 前置条件（脚本逐项检查，失败会给出提示）

1. `gh` CLI 已登录（`gh auth status`）。
2. 产物已构建：`cargo xtask dist`。产物目录或 zip 缺失时先构建再重试。
3. 当前分支无未推送提交（tag 打在远端分支头上，必须先 `git push`）。
4. 同名 tag / release 尚不存在（防止重复发布）。

## 执行

```powershell
powershell -ExecutionPolicy Bypass -File "D:\Github\Matcha\.agents\skills\matcha-publish\publish.ps1"
```

可选参数：

- `-Notes "<markdown>"`：自定义 release 说明；默认 `Matcha v<version>`。
- `-GenerateNotes`：用 GitHub 自动生成的提交说明。
- `-Draft` / `-Prerelease`：创建草稿 / 预发布。
- `-DryRun`：只做全部检查并打印发布计划，不打 tag、不创建 release、不升级版本号。适合先验证。

## 成功后

- 脚本输出 release URL、已发布版本、下一个版本号。
- `config.json` 与 `Cargo.toml` 的版本号变更留在工作区。**不要擅自 commit/push**，向用户报告变更内容，由用户决定何时提交（遵循 AGENTS.md 的发布纪律）。
- 提醒用户：下次发布前先 `cargo xtask dist` 生成新版本目录。
