# Linux release preparation

Status: the x86-64 AppImage has been built on Ubuntu and manually tested successfully. The Ubuntu-server build verified copyright notices for all 178 bundled ELF libraries with 0 unresolved libraries. Version 0.2.1 is published on GitHub. Future Linux candidates are built on the project's Ubuntu server.

Build on Linux, using the same CPU architecture as the intended users. For an x86-64 AppImage, Ubuntu 22.04 is a suitable starting baseline. Building on a newer distribution can make the result depend on libraries unavailable on older systems. See [Tauri's AppImage guidance](https://v2.tauri.app/distribute/appimage/).

## Build prerequisites

Install stable Rust, the Tauri 2 CLI, Node.js for the frontend tests, and the [Tauri Linux system dependencies](https://v2.tauri.app/start/prerequisites/#linux). The app also needs yt-dlp, FFmpeg, and FFprobe for hands-on download testing; see the main README for setup.

Use the reviewed source including local fixes, not an older checkout. Build on the Ubuntu server from the exact source commit intended for release.

## Release verification

- Run Rust and JavaScript tests on Linux.
- Build an AppImage and inspect its extracted contents.
- Check for bundled `libwayland-client.so*`, which caused graphics compatibility problems in an earlier project. Verify the produced contents rather than assuming an exclusion environment variable was honored.
- Confirm the generated license audit reports 0 unresolved libraries.
- Generate a SHA-256 checksum for the final artifact.
- Test the actual AppImage on the intended desktop: launch, paste, folder chooser, formats, download, conversion, cancellation, saved preferences, themes, and window growth.
- Record the build distribution, CPU architecture, tested distributions, and any limitations.

A successful build is not yet evidence that the AppImage runs correctly on another distribution. Publish only the artifact that was tested.

## Build on the Ubuntu server

After installing the prerequisites, run this from the repository root:

```sh
./scripts/build-linux-appimage.sh
```

The script runs the Rust and JavaScript checks, builds the AppImage, removes the incompatible bundled Wayland client library if necessary, packages and verifies the Debian copyright notice for every bundled ELF library, reinspects the result, and creates `target/release/bundle/appimage/linux-candidate.tar.gz`.

Unpack that archive on Linux. The tar archive preserves the AppImage's executable permission. In the extracted folder, run:

```sh
sha256sum -c SHA256SUMS
```

Then open the AppImage on your Linux desktop. Install yt-dlp, FFmpeg, and FFprobe separately. If the desktop cannot mount AppImages, use the AppImage runtime's `--appimage-extract-and-run` option from a terminal.

The build script checks tests, packaging, the absence of the bundled Wayland client, and library copyright notices. It fails closed if a bundled library cannot be matched exactly to an installed Debian package by ELF build ID. Review `appimage-license-audit.txt` and test the real desktop application before distributing it.
