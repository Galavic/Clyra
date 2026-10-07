# Packaging

## Linux (implemented)

```sh
scripts/package-linux.sh            # release build (thin LTO, stripped)
PROFILE=debug scripts/package-linux.sh   # fast smoke package
```

Produces `target/package/clyra-<version>-linux-<arch>.tar.gz` containing:

- `clyra` — the binary (headed by default; `clyra headless` runs the engine alone)
- `clyra.desktop` — XDG desktop entry
- `clyra.png` — 1024×1024 Clyra app icon
- `install.sh` — installs into `~/.local/{bin,share/applications,share/icons}`

The release profile in the root `Cargo.toml` sets `lto = "thin"` and
`strip = "symbols"` for distribution builds.

## macOS

```sh
scripts/package-macos.sh    # → target/package/clyra-<version>-macos-<arch>.dmg
```

Builds the release binary, assembles `Clyra.app` (Info.plist + icns), ad-hoc
signs it (set `CODESIGN_IDENTITY` for a real Developer ID), and wraps it in a
dmg. Updates apply over the running bundle wherever it lives, so installs
that predate the rebrand keep working inside their existing `Clyra.app`
until reinstalled. CI runs this on tags
(`.github/workflows/release.yml`). The manual steps it automates, for reference
(run on a macOS host — gpui needs Metal; no cross-build from Linux):

1. Build the universal (or per-arch) binary:
   ```sh
   cargo build --release -p clyra --target aarch64-apple-darwin
   cargo build --release -p clyra --target x86_64-apple-darwin
   lipo -create -output clyra \
     target/aarch64-apple-darwin/release/clyra \
     target/x86_64-apple-darwin/release/clyra
   ```
2. Assemble the bundle:
   ```sh
   mkdir -p Clyra.app/Contents/{MacOS,Resources}
   cp clyra Clyra.app/Contents/MacOS/clyra
   sed "s/__VERSION__/$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')/" \
     dist/macos/Info.plist > Clyra.app/Contents/Info.plist
   ```
3. Icon: generate `clyra.icns` from `dist/macos/icon-1024.png` (the macOS-shaped
   variant of the artwork — squircle mask, margins, and shadow pre-baked, since
   `sips` can't apply an alpha mask) and place it at
   `Clyra.app/Contents/Resources/clyra.icns`:
   ```sh
   mkdir clyra.iconset && sips -z 256 256 dist/macos/icon-1024.png --out clyra.iconset/icon_256x256.png
   iconutil -c icns clyra.iconset -o Clyra.app/Contents/Resources/clyra.icns
   ```
4. Sign + notarize (required for distribution):
   ```sh
   codesign --deep --force --options runtime --sign "Developer ID Application: …" Clyra.app
   xcrun notarytool submit Clyra.zip --keychain-profile … --wait
   xcrun stapler staple Clyra.app
   ```
5. Ship as a `.dmg` (`hdiutil create -volname Clyra -srcfolder Clyra.app -ov -format UDZO Clyra.dmg`).
