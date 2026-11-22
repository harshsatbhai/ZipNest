// ─────────────────────────────────────────────────────────────
// ZipNest website configuration: the only file you need to edit.
// ─────────────────────────────────────────────────────────────
window.ZIPNEST_CONFIG = Object.freeze({
  // GitHub "owner/repo" that hosts the releases (installers are attached to GitHub Releases).
  repo: "YOUR_GITHUB_USERNAME/zipnest",

  // Shown until the live version is fetched from GitHub (and if GitHub is unreachable).
  fallbackVersion: "1.0.0",

  // Installer file names. These match .github/workflows/release.yml. Change both together.
  assets: {
    windows: "ZipNest-windows-x64-setup.exe",
    macos: "ZipNest-macos-universal.dmg",
    deb: "ZipNest-linux-amd64.deb",
    appimage: "ZipNest-linux-x86_64.AppImage",
    checksums: "SHA256SUMS.txt",
  },
});
