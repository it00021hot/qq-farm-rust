# Release verification checklist

Use after pushing a `v*` tag (or `workflow_dispatch` with a tag).

## Secrets (once)

GitHub repo **Settings → Secrets and variables → Actions**：

- `TAURI_SIGNING_PRIVATE_KEY` — minisign 私钥全文（与 `tauri.conf.json` 里 `plugins.updater.pubkey` 配对）
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — 生成密钥时的密码。当前密钥无密码时**不要设这个 Secret**（Windows 会把空字符串当成错误密码；macOS 则当作无密码）。

本地私钥在 `crates/qq-farm-desktop/.tauri/`（gitignore，勿提交）。丢失后无法给旧客户端签名更新，只能换密钥并让用户重装。

## CI / Release page

- [ ] Actions workflow **Release** is green (macos + windows + SHA256SUMS)
- [ ] GitHub Release for the tag includes:
  - Windows NSIS installer (`.exe`)
  - macOS `.dmg`
  - updater artifacts (`.sig` / `.app.tar.gz` / nsis zip) and `latest.json`
  - `SHA256SUMS`
- [ ] `assets/tsdk.wasm` was present in the checkout (workflow fails otherwise)

## Windows

- [ ] Run the installer (per-user, no admin prompt)
- [ ] App starts; tray shows **检查更新**
- [ ] Closing the window hides to tray; **退出** quits
- [ ] Clean machine can load TSDK and log in (no source tree required)
- [ ] Publish a higher version tag → **检查更新** downloads, replaces, relaunches

## macOS

- [ ] Open DMG, drag to Applications (or `~/Applications`)
- [ ] First open may need Privacy & Security allow (ad-hoc signed)
- [ ] App menu **应用 → 检查更新** and tray work against a newer Release
- [ ] Dock click while hidden shows the window; red traffic light hides to tray

## Local smoke (optional)

```bash
pnpm -C desktop-ui i
cd crates/qq-farm-desktop && cargo tauri build
```
