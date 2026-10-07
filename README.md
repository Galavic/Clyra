# Clyra

One window for every coding agent you run — Claude Code, Codex, Cursor, Devin, Grok, Hermes, Pi, Antigravity — on your own hardware first, with optional sync between your devices.

*English | [简体中文](README.zh-CN.md)*

![Clyra driving a Claude Code session with a live branch diff sidebar](apps/landing/public/assets/app-screenshot.jpg)

Each of your devices runs a small engine that keeps its sessions right there. A fresh install starts local-only: no account, no network needed.

## Desktop installers (Windows, macOS, Linux)

Build and download the desktop installers through [Clyra installers](https://github.com/Galavic/Clyra/actions/workflows/installers.yml).
Windows uses a setup `.exe`, macOS uses a `.dmg` for Apple silicon or Intel, and Linux
uses a `.deb` or a portable `.tar.gz` for x64 or ARM64. See [installer instructions](dist/INSTALLERS.md).

## Run locally (Linux)

```bash
# Install the downloaded desktop package first:
sudo apt install ./clyra-0.2.83-linux-x86_64.deb
clyra status
```

Open Clyra from your application menu. No sign-in or sync configuration is required
for local use. To install an always-on user daemon, run `clyra daemon install`.

The desktop sidebar browser also needs the [Linux browser runtime](docs/reference/linux-browser.md).

Day-to-day:

```bash
clyra status      # local/synced mode and engine status
clyra update      # update to the latest release
clyra daemon start|stop|restart|status
```

## Optional multi-device sync

Sign in only when you want to open your account's synced workspace. Authentication changes the profile selected by the next engine start, so stop the daemon before changing it:

```bash
clyra daemon stop
clyra login
clyra daemon start
```

You can then start an agent on one synced device and follow or drive it from another. An always-on machine such as a VPS can keep those agents working after you close your laptop.

Devices signed in to the same synced account are trusted with remote workspace access. A device controlling a workspace on another device can list, read, and write its files; enabling `Show ignored files` also makes gitignored files such as `.env` available remotely. `.git` is always excluded. Only sign in devices you trust with the full contents of your workspaces.

Signing in does not upload, move, or import existing local sessions. Local sessions and their attachments remain under the local profile and reappear when you return to local-only mode:

```bash
clyra daemon stop
clyra logout
clyra daemon start
```

`clyra login` and `clyra logout` refuse to modify credentials while an engine owns the data directory. The desktop app follows the same next-restart profile boundary.

On macOS: use the desktop release, or build `clyra` from source and run `clyra daemon install` to install the launchd service.

On Windows: open the setup installer. A portable ZIP is also available; extract it
and run `clyra.exe`. See the [development notes](docs/reference/windows-development.md) for source builds.

---

Developing or curious how it works? [![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/Galavic/Clyra) or check out [ARCHITECTURE.md](ARCHITECTURE.md).

Licensed under the [MIT License](LICENSE).
