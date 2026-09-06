# macOS release candidate

Build this package on macOS from the repository root:

```sh
cargo tauri build --bundles app dmg -- --locked
```

The current configuration signs the complete app bundle ad hoc (`signingIdentity: "-"`). This does not provide Developer ID signing or Apple notarization.

## Install and test

1. Open the generated DMG under `target/release/bundle/dmg/`.
2. Copy **YT-DLP Rusty GUI.app** to Applications. Quit any older running copy before opening this one.
3. Install yt-dlp, FFmpeg, and FFprobe as described in the README. These are not bundled.
4. If macOS blocks this unnotarized build, review **System Settings → Privacy & Security → Open Anyway** for the app you just attempted to open. See [Apple’s instructions](https://support.apple.com/en-us/102445).
5. Test a video download, an MP3 download, progress, cancellation, saved preferences, themes, and window growth after fetching formats.

Test the actual packaged app, not `cargo tauri dev`. A second Apple-silicon Mac is a useful additional check. Intel Macs require a separate or universal build.

## Verification record

Verified on 2026-09-06:

- Version: 0.2.1; architecture: arm64 (Apple silicon).
- DMG: `target/release/bundle/dmg/YT-DLP Rusty GUI_0.2.1_aarch64.dmg`.
- `hdiutil verify`: passed.
- `codesign --verify --deep --strict --verbose=2`: passed for both the build output and the app mounted from the DMG.
- Signature: ad hoc, with sealed resources and hardened runtime; no Developer ID or notarization.
- Tests: 19 JavaScript tests and 11 Rust tests passed.
- SHA-256: `c27719fe80adb5f8de1a72a93eb03c505a07f53b127f4b395b083c04121c5cf5`.

Packaged-app startup and interaction testing remain a manual step for this rebuilt version. The Linux x86-64 AppImage has separate verification; Intel macOS is not currently offered. This candidate has not been uploaded or published.
