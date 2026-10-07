# Clyra desktop installers

All installers include the compiled desktop app, embedded animated Dots, and font licenses.
Generated files are written to `target/package/`.

| Platform | Download | Architectures |
| --- | --- | --- |
| Windows | `clyra-VERSION-windows-x86_64-setup.exe` | x64 |
| macOS | `clyra-VERSION-macos-ARCH.dmg` | Apple silicon (`arm64`), Intel (`x86_64`) |
| Linux | `clyra-VERSION-linux-ARCH.deb`, `.tar.gz` | x64 (`x86_64`), ARM64 (`aarch64`) |

## Build

Build each installer on its corresponding OS. GitHub Actions → **Clyra installers** →
**Run workflow** builds all five architectures and offers each download in the run's
Artifacts section. This workflow does not publish a release or contact the legacy update feed.
The source changes must be committed in the Clyra repository before running it.

- Windows: install Inno Setup 6 or 7 and the Windows SDK, then run
  `./scripts/build-windows-installer.ps1`. Set `GPUI_FXC_PATH` to the SDK's `x64/fxc.exe`
  if it is not discovered by GPUI. `-CompilerPath` accepts a portable `ISCC.exe`.
  `-SkipBuild` packages an already compiled release. An optional HTTPS `-ReleasesUrl`
  configures the managed Windows update feed.
  The setup also includes the x64 Visual C++ runtime from the installed Visual Studio
  redistributable directory alongside the executable, using Microsoft's
  [app-local deployment](https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files).
- macOS: run `bash scripts/package-macos.sh` on each architecture. The existing
  `CODESIGN_IDENTITY` and `NOTARY_*` variables enable Developer ID signing and notarization.
  The installer workflow uses ad-hoc signing unless a separate signed release is built.
- Linux: install the development libraries listed in `installers.yml` and `dpkg-dev`,
  then run `bash scripts/package-linux.sh`. The `.deb` records the linked library
  requirements automatically. Other distributions can use the archive's `install.sh`
  with their corresponding runtime libraries installed.

## Install

- Windows: open the setup. It installs `clyra.exe` in the current user's
  `%LOCALAPPDATA%\Programs\Clyra`, creates a Start menu entry, and offers a desktop shortcut.
  Uninstall through Windows Settings. User data in `%LOCALAPPDATA%\Clyra` is preserved.
- macOS: open the `.dmg` and drag Clyra into Applications. An ad-hoc signed build
  requires allowing the app through macOS Privacy & Security on first launch.
- Debian/Ubuntu: `sudo apt install ./clyra-VERSION-linux-ARCH.deb`.
- Other Linux distributions: extract the `.tar.gz` and run its `./install.sh`.

For an isolated Windows payload check, run setup with
`/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /EXTRACTONLY=1 /DIR="ABSOLUTE_TEST_DIRECTORY"`.
This extracts the payload without shortcuts or uninstall registration and does not launch Clyra.
