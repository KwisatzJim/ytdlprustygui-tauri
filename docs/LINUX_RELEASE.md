# Linux release preparation

Status: no Linux package has been built or tested in this release pass.

Build on Linux, using the same CPU architecture as the intended users. For an x86-64 AppImage, Ubuntu 22.04 is a suitable starting baseline. Building on a newer distribution can make the result depend on libraries unavailable on older systems. See [Tauri's AppImage guidance](https://v2.tauri.app/distribute/appimage/).

## Build prerequisites

Install stable Rust, the Tauri 2 CLI, Node.js for the frontend tests, and the [Tauri Linux system dependencies](https://v2.tauri.app/start/prerequisites/#linux). The app also needs yt-dlp, FFmpeg, and FFprobe for hands-on download testing; see the main README for setup.

Use the reviewed source including local fixes, not an older remote checkout. The selected build environment is GitHub Actions on Ubuntu 22.04, using the source commit chosen for the workflow run.

## Planned verification

- Run Rust and JavaScript tests on Linux.
- Build an AppImage and inspect its extracted contents.
- Check for bundled `libwayland-client.so*`, which caused graphics compatibility problems in an earlier project. Verify the produced contents rather than assuming an exclusion environment variable was honored.
- Check bundled library license notices before distributing the image.
- Generate a SHA-256 checksum for the final artifact.
- Test the actual AppImage on the intended desktop: launch, paste, folder chooser, formats, download, conversion, cancellation, saved preferences, themes, and window growth.
- Record the build distribution, CPU architecture, tested distributions, and any limitations.

A successful build is not yet evidence that the AppImage runs correctly on another distribution. Publish only the artifact that was tested.

## Run the GitHub Actions build

The first build runs when the source-only `codex/linux-release` branch is pushed. Once the workflow is on the default branch, it can also be run from **Actions → Linux AppImage candidate → Run workflow**. It has read-only repository permissions and does not create a GitHub release.

Download the resulting `rusty-gui-linux-x86_64-...` artifact from the run page within 14 days. Unzip the artifact and unpack `linux-candidate.tar.gz` on Linux. The tar archive preserves the AppImage's executable permission. In the extracted folder, run:

```sh
sha256sum -c SHA256SUMS
```

Then open the AppImage on your Linux desktop. Install yt-dlp, FFmpeg, and FFprobe separately. If the desktop cannot mount AppImages, use the AppImage runtime's `--appimage-extract-and-run` option from a terminal.

The workflow checks tests, packaging, and the absence of the bundled Wayland client. If that check fails, stop and fix the package before distributing it. Library-license review and real desktop testing remain release requirements; an Actions artifact is only a test candidate.
