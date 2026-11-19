---
name: matcha-publish
description: Matcha 本地构建打包；用户明确要求发布到 GitHub Releases 时，整理自上一正式版以来的中文变更说明，构建、按授权推送并核验发行包。
metadata:
  version: "1.1.0"
---

# Matcha 发布

**先分清请求。**「发布」「打个包」「本地发布」只走本地流程。只有用户明确要求 **GitHub / GitHub Releases / 远程发布**，才允许打 tag 和创建 Release。讨论、总结或修改本 skill 不算发布授权。远程发布也不等于授权推送尚未发布的分支提交。

## 本地：一条命令，不做远程预检

```powershell
cargo xtask dist
```

即使没有代码变动也直接运行。列出 stdout 最后一行指向的目录，报告版本和四个文件：`matcha.exe`、`matcha.com`、`matcha-comp.exe`、`README.txt`。明确说明「未打 tag、未创建 GitHub Release」。不查 gh 登录、不检查上游、不运行发布脚本；只有用户另要 ZIP 时才压缩。

## GitHub：先确定发布什么，再写入远端

下例在仓库根目录运行；Windows 脚本策略受限时可用 `powershell -NoProfile -ExecutionPolicy Bypass` 启动当前流程。分步检查退出码；跨工具调用要保留同一组已确认的版本、SHA 和说明。只要预演时止于 `-DryRun`，不为通过检查而推送。

### 1. 固定版本、目标提交和比较基线

```powershell
$skill = '.agents/skills/matcha-publish'
$cfg = Get-Content "$skill/config.json" -Raw -Encoding UTF8 | ConvertFrom-Json
$repo = $cfg.repo; $remote = $cfg.remote; $version = $cfg.version
$tag = "$($cfg.tagPrefix)$version"; $sha = git rev-parse HEAD
$dist = Join-Path $cfg.distRoot "matcha-$version-$($cfg.platform)"
$branch = git branch --show-current
$dest = git for-each-ref '--format=%(upstream:remoteref)' "refs/heads/$branch"
$upstreamRemote = git for-each-ref '--format=%(upstream:remotename)' "refs/heads/$branch"
if (!$branch -or !$dest -or $upstreamRemote -ne $remote) { throw '请先确认上游分支' }
$notesFile = Join-Path ([IO.Path]::GetTempPath()) "matcha-release-$sha.md"
git status --short --branch
git remote get-url --push $remote
gh auth status
git fetch $remote
git rev-list --left-right --count '@{u}...HEAD'   # 左：落后；右：领先
$oldHead = git ls-remote --exit-code $remote $dest   # 确认前记录，推分支前重查
$prev = gh release view --repo $repo --json tagName --jq .tagName
```

- `config.json` 的 `version` 是**本次要发的版本**，必须与 `Cargo.toml` 的 `[workspace.package] version` 一致。要求跳版时先准备两个文件并核对 `Cargo.lock`。工作区不干净（包括上次遗留的升版）、无上游、目标仓库/上游分支不符、落后或分叉：列出原因让用户决定，不代为提交、合并或强推。
- 推分支前先排除同版本冲突：`git tag -l $tag`、`git ls-remote --exit-code --tags $remote "refs/tags/$tag"`（退出码 2 才是无匹配）、`gh release view $tag --repo $repo`（明确 not found 才是不存在）。遇到同名对象、认证或网络异常就停。
- `$prev` 取 GitHub 的 **Latest 正式版**，不取排序最大的本地 tag。可用 `gh release list --repo $repo --json tagName,isLatest,isDraft,isPrerelease,publishedAt` 核对；仅有预发布、Latest 与最近正式发布不一致时先确认基线。用户指定基线时按指定版比较。缺少基线 tag 时只 fetch 该 tag，核对其远端指向。确属首次发布时，按首发写说明，跳过下面的区间命令，不造对比链接。

### 2. 从实际提交写中文说明，并构建

```powershell
git merge-base --is-ancestor "refs/tags/$prev" $sha  # 非零则不要使用此范围
$range = "${prev}..$sha"
git log --oneline $range
git diff --stat $prev $sha
git diff $prev $sha -- CHANGELOGS.md
$count = git rev-list --count $range
```

按用户可感知行为合并相关提交，每条写清「哪里变了、现在怎样」；不照抄整份日志，不把纯升版当成新功能。按需看具体 diff。正文以 `## 相较 <上版> 的变化` 开头，末尾附 `[查看全部 <数量> 个提交](https://github.com/<仓库>/compare/<上版>...<本版>)`。范围为空先告知，不虚构变动。[v0.1.1 的实际说明](https://github.com/slb1988/Matcha/releases/tag/v0.1.1) 仅作格式示例。将定稿存为工作区外的 UTF-8 文件 `$notesFile`，留到发布后比对。

按 diff 运行相关测试（例如 GUI 改动用 `cargo test --locked -p bc-gui --lib --quiet`），记录通过/跳过数。运行 `cargo xtask dist`，确认输出目录就是 `$dist`，用 `Get-ChildItem -Force $dist` 核对**恰好**上述四个非空文件，没有多余条目，且构建未弄脏工作区。测试或构建失败不发布。xtask 只组装目录；**不要手工维护或复用旧 ZIP**。

### 3. 需要分支推送时，只推用户确认的提交

- 已同步：明确的 GitHub 发布请求即可继续；**不需要推分支**。
- 仅领先：用 `git log --oneline '@{u}..HEAD'` 列出待推的 SHA 和标题，给出目标远端/分支、版本、说明草稿、构建/测试结果，明确「尚未打 tag 或创建 Release」。如果未获分支推送授权，停下问「允许推送这些提交并按此说明发布吗？」。同一请求中回复「可以，发布推送」即为确认，不重复询问。
- 取得授权后重查工作区、`HEAD`、配置和远端分支头；和确认时不一致就停下重新定范围。只快进推送已确认的 SHA 到已确认的上游引用：`git push --no-follow-tags $remote "${sha}:$dest"`。不使用 `--force`、`--all` 或 `--tags`。

### 4. 预检、发布、只读验收

发布脚本对**本地 HEAD** 打 tag，不能只相信缓存的 `origin/...` 或 ahead 数。确认等待后可重跑 `cargo xtask dist`，从同一 SHA 重新组装目录。保留之前的 `$tag`、`$sha`、`$dist` 和说明，不在脚本升版后重新推算。

```powershell
$headRef = git ls-remote --exit-code $remote $dest
if ($LASTEXITCODE -ne 0 -or ($headRef -split '\s+')[0] -ne $sha) { throw '远端分支不是待发布提交' }
if ((git rev-parse HEAD) -ne $sha -or (git status --porcelain)) { throw '构建源已变化' }
& "$skill/publish.ps1" -DryRun
if ($LASTEXITCODE -ne 0) { throw '预检未通过' }
```

**预演到此为止。** 只有正式发布且预检通过，才执行：

```powershell
$notes = Get-Content -LiteralPath $notesFile -Raw -Encoding UTF8
if ([string]::IsNullOrWhiteSpace($notes)) { throw '发布说明为空' }
& "$skill/publish.ps1" -Notes $notes
if ($LASTEXITCODE -ne 0) { throw '先检查远端状态，不盲目重试' }
& "$skill/verify-release.ps1" -Tag $tag -Commit $sha -DistDir $dist -Notes $notes
```

- `-DryRun` 不压缩、不打 tag、不写 Release、不升版。同名 tag/Release 或远端查询异常都应停下。正式运行才由 `publish.ps1` 重新压缩目录、推注解 tag 并上传。传中文多行 Markdown 用变量，不把正文拼进 shell 命令；从 Git Bash 调 PowerShell 时也可用环境变量传给 `-Notes`。
- 用户要求「带上变动点」就用 `-Notes`，不要叠加会覆盖它的 `-GenerateNotes`。只有明确要 GitHub 自动说明时才用后者，核验时省略 `-Notes`（仍检查正文非空）。草稿/预发布按要求将 `-Draft` / `-Prerelease` 同时传给发布和核验脚本。
- `verify-release.ps1` **只读**：核对远端注解 tag 解引用后的 SHA、Release 状态和正文、附件名称/上传状态/大小及每个 SHA-256。默认是 ZIP + 四个散件；`includeRawFiles=false` 时只验 ZIP。若 GitHub 没返回 digest，应下载到临时目录比对哈希，否则只报「未核验」。创建或核验失败时，报告已推的分支/tag、是否已有 Release、哪项未通过；不删除或覆盖远端 tag，不盲目重发或编辑 Release。
- 发布后再读一次 `git ls-remote --exit-code $remote $dest`；若分支又前进，分别说明分支头和已验证的 tag，不挪动 tag。核验通过再报告 Release URL、ZIP 直链、发布提交、主要变动和核验结果。成功后脚本将补丁号 +1（`config.json`、默认也同步 `Cargo.toml`）；**升版留在工作区，勿擅自 commit/push**。`Cargo.lock` 在下次 cargo 构建时刷新；下次发版前重新运行 `cargo xtask dist`。

维护核验脚本时可离线运行 `& "$skill/tests/verify-release.tests.ps1"`；测试不调用真实 git/gh，也不需要发布权限。
