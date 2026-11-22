# ZipNest

**Zip and unzip files and whole project folders, 100% offline.**
A tiny, fast desktop app for Windows, macOS and Linux, built with Rust + Tauri, plus its download website on Render.

```
zipnest/
├── crates/zipnest-core/     Rust zip engine (no UI, no network). Fully tested.
├── app/                     Desktop app (Tauri 2)
│   ├── src/                 UI: React + TypeScript
│   └── src-tauri/           Rust shell: commands, permissions, installer config
├── website/                 Download site (static HTML/CSS/JS, no build step)
├── render.yaml              Render Blueprint for the website (+ security headers)
└── .github/workflows/       CI + release pipeline (builds all installers)
```

## Features

| | |
|---|---|
| **Offline** | No network code and no network permissions. No telemetry, no account. |
| **Codebase-aware** | Honours `.gitignore`. Skips `node_modules`, `.git`, `__pycache__`, `.venv`, `.next`, `.DS_Store`… (each filter can be switched off) |
| **AES-256** | Optional password protection that works with 7-Zip, WinRAR and others |
| **Safe extraction** | Blocks zip-slip path traversal, never creates symlinks, detects zip bombs, sanitises Windows-invalid names, strips setuid bits |
| **Never overwrites** | Extracts into a new folder (`project (2)` if needed). ZIPs are written to a temp file and renamed into place when done. Cancelled or failed jobs are cleaned up. |
| **Streaming** | A 2.1 GB folder round-trips with ~13 MB of RAM. Zip64 handles files over 4 GB. |
| **Native feel** | Drag & drop, native file dialogs, "Open with ZipNest" for `.zip`, dark mode, ⌘/Ctrl+1/2 |
| **Small** | ~4 MB installer (uses the OS's own web view instead of bundling Chromium) |

---

## 1 · Run it locally (development)

**Prerequisites**

| OS | Install |
|---|---|
| all | [Rust](https://rustup.rs) (stable) and [Node.js](https://nodejs.org) 24 LTS (20+ works) |
| macOS | `xcode-select --install` |
| Windows | [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with "Desktop development with C++". WebView2 is preinstalled on Windows 10/11. |
| Debian/Ubuntu | `sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf build-essential` |

```bash
cd app
npm install
npm run tauri dev        # opens the app with hot reload
```

Run the engine tests (zip-slip, passwords, cancel, filters, permissions…):

```bash
cargo test -p zipnest-core
```

Build installers for **your current OS**:

```bash
cd app
npm run tauri build      # output: target/release/bundle/
```

---

## 2 · Publish installers (GitHub Releases)

You can't build a macOS app on Windows (or the other way round), so GitHub Actions builds all three on real Windows, macOS and Linux machines.

1. Push this project to https://github.com/harshsatbhai/ZipNest.
2. Set the version in **`app/src-tauri/tauri.conf.json`** (and optionally `Cargo.toml` / `app/package.json`).
3. Tag and push:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```
4. **Actions → Release** builds everything (~15 min) and creates a GitHub Release with:

   | File | Platform |
   |---|---|
   | `ZipNest-windows-x64-setup.exe` | Windows 10/11 installer (per-user, includes uninstaller) |
   | `ZipNest-macos-universal.dmg` | macOS 10.15+, Apple Silicon + Intel |
   | `ZipNest-linux-amd64.deb` | Debian / Ubuntu / Mint |
   | `ZipNest-linux-x86_64.AppImage` | Any Linux distro |
   | `SHA256SUMS.txt` | Checksums for verification |

   File names are stable, so the website's `…/releases/latest/download/<file>` links always point to the newest version.

> The release job fails on purpose if the tag (`v1.0.0`) doesn't match the version in `tauri.conf.json`.

---

## 3 · Deploy the website on Render

1. `website/config.js` is already set to `repo: "harshsatbhai/ZipNest"`. If you rename or move the repo, update it there and in `website/.well-known/security.txt`.
2. Commit and push to `main`.
3. In Render: **New → Blueprint**, then pick your repo. Render reads `render.yaml` and creates a free static site called `zipnest`, published at **https://zipnest.onrender.com**.
4. Optional: **Settings → Custom Domains** to add e.g. `zipnest.app`. Render issues the HTTPS certificate automatically.

The site auto-deploys only when `website/**` or `render.yaml` change.

**Website security** (set in `render.yaml`): strict Content-Security-Policy (only same-origin scripts, no inline JS, only `api.github.com` for the live version number), HSTS, `X-Frame-Options: DENY`, `nosniff`, a locked-down `Permissions-Policy`, COOP/CORP. No cookies, no analytics, no third-party fonts. Installers are served from GitHub Releases, not Render, and checksums are published with each release.

---

## 4 · Install & uninstall (what users see)

| | Install | Uninstall |
|---|---|---|
| **Windows** | Run the `.exe`. No admin rights needed. | Settings → Apps → ZipNest → Uninstall |
| **macOS** | Open the `.dmg` and drag to Applications | Drag ZipNest to the Trash |
| **Debian/Ubuntu** | `sudo apt install ./ZipNest-linux-amd64.deb` | `sudo apt remove zip-nest` |
| **AppImage** | `chmod +x` and run | Delete the file |

### Code signing (optional, removes "unknown developer" warnings)

Unsigned builds work, but Windows SmartScreen and macOS Gatekeeper show a warning on first launch. The website explains how to get past it.

* **macOS:** join the Apple Developer Program ($99/yr), then add the repo secrets `APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (app-specific password) and `APPLE_TEAM_ID`. The release workflow then signs and notarizes automatically.
* **Windows:** get an OV/EV code-signing certificate, or use Azure Trusted Signing, and follow the [Tauri Windows signing guide](https://v2.tauri.app/distribute/sign/windows/).

---

## Security model (desktop app)

* **No network:** the Rust code has no HTTP client, and the webview CSP only allows the local IPC channel (`connect-src ipc:`).
* **Minimal permissions** (`app/src-tauri/capabilities/default.json`): open/save dialogs and "reveal in folder" only. There's no shell, no generic filesystem API and no URL opening from the UI.
* **Frozen JS prototypes**, no remote content, and no `eval` (`script-src 'self'`).
* All archive parsing happens in Rust (memory-safe) through the well-maintained `zip` crate.

## Project tips

* Change the excluded junk folders in `crates/zipnest-core/src/compress.rs` (`DEFAULT_EXCLUDES`).
* Change the zip-bomb limits in `ExtractOptions::default()` (`crates/zipnest-core/src/extract.rs`).
* There's a quick CLI for testing: `cargo run --release -p zipnest-core --example zipnest_cli -- zip out.zip ./folder`
* Regenerate the app icons from the SVG: `cd app && npx tauri icon ../assets/logo.svg -o src-tauri/icons`

## License

MIT © 2026 [Harsh Satbhai](https://github.com/harshsatbhai). Created by Harsh Satbhai.
