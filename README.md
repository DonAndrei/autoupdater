# AutoUpdater

Tray app that finds outdated apps on your computer and lets you pick which ones to update. Handy for a laptop or PC you haven't touched in a while.

- **Windows:** uses [winget](https://learn.microsoft.com/windows/package-manager/winget/)
- **macOS:** uses [Homebrew](https://brew.sh) (formulae + casks) and [mas](https://github.com/mas-cli/mas) for Mac App Store apps

Built with [Tauri 2](https://tauri.app), Rust and React.

## Features

- Scan for outdated apps, filter, select, update in one click
- Live progress and logs per app
- Tray icon: open, check for updates, quit; closing the window keeps it in the tray
- Notifications when a check or update finishes

## Develop

Requirements: Rust, Node.js 18+, and on Windows the Visual Studio C++ build tools.

```bash
npm install
npm run tauri dev
```

Run parser tests:

```bash
cd src-tauri && cargo test
```

Build installers (output in `src-tauri/target/release/bundle`):

```bash
npm run tauri build
```

Notifications show the app name only when it's installed; in dev mode Windows shows them as PowerShell.
