# YT-DLP Rusty GUI

A desktop front end for [yt-dlp](https://github.com/yt-dlp/yt-dlp), built with Rust and Tauri. Download video with audio to MP4, or extract audio to MP3.

## Features

- Choose video and audio formats before downloading video.
- Save a preferred video resolution, audio quality, audio language, and output folder.
- Switch between Light, Dark, and System appearance.
- See live download progress and merging/conversion activity.
- Cancel a download and keep partial files for a later retry.

**Resolution:** selects the highest available resolution at or below your preference. If all available resolutions are higher, it selects the lowest. You can override the selected format.

**Audio quality:** High is the default. For video downloads, High/Low selects among the available audio formats within the language preference. For MP3, it controls conversion quality using the best available source; it cannot improve the original recording. The saved audio-language preference currently applies to the video format selector, not automatic MP3 downloads.

## Required tools

The app does not bundle these tools. Install them separately and make them available on your `PATH` (the folders your operating system searches for programs):

- **yt-dlp:** retrieves media. Follow its [installation instructions](https://github.com/yt-dlp/yt-dlp#installation).
- **FFmpeg and FFprobe:** merge video/audio and convert audio to MP3. Install the actual programs, not a Python package with a similar name. See [yt-dlp dependencies](https://github.com/yt-dlp/yt-dlp#dependencies).
- **For full YouTube support:** follow the [yt-dlp EJS setup guide](https://github.com/yt-dlp/yt-dlp/wiki/EJS) for a supported JavaScript runtime and the challenge-solving component.

Check the core tools in a terminal:

```sh
yt-dlp --version
ffmpeg -version
ffprobe -version
```

The app checks yt-dlp at startup and verifies that FFmpeg and FFprobe can run before each download. If either is missing or fails its check, the app explains what needs attention before fetching media. Each check has a five-second timeout. On macOS and Linux, the app reads your login-shell PATH at startup so desktop-launched builds can find Homebrew and other user-installed tools.

## Build and run

Install current stable Rust using [rustup](https://rustup.rs/), Git, and the [Tauri prerequisites for your operating system](https://v2.tauri.app/start/prerequisites/). These include platform build tools and system libraries; installing Rust alone is not sufficient.

Install the Tauri command-line tool:

```sh
cargo install tauri-cli --version "^2" --locked
```

Get this project:

```sh
git clone https://github.com/KwisatzJim/ytdlprustygui-tauri.git
cd ytdlprustygui-tauri
```

Run the development app:

```sh
cargo tauri dev
```

Build the app and installers for your current operating system:

```sh
cargo tauri build -- --locked
```

With the default Cargo target directory, output is under **`target/release/bundle/`** in this repository. There is no `src-tauri/` subdirectory. Explicit cross-compilation targets or a custom `CARGO_TARGET_DIR` change the output location.

The frontend is plain HTML, CSS, and JavaScript; no `npm install` or frontend build step is required. Node.js is needed only to run the JavaScript tests below, unless you separately configure it as yt-dlp's JavaScript runtime.

## Using the app

1. Paste a video URL and choose an output folder.
2. Set your quality preferences and download type.
3. For **Video+Audio**, click **Fetch Formats**, then review or change the selected formats. For **Audio Only**, fetch is not required.
4. Click **Download** and watch the status. Video and audio transfers may report separate percentages; merging or conversion can continue after a transfer reaches 100%.
5. Use **Cancel Download** to stop an active download. Partial files remain; retrying the same URL, folder, and formats may resume the transfer where yt-dlp supports it.

Changing the URL clears previously fetched formats. Format tables start collapsed and can be expanded for more detail. The window grows downward as controls appear, up to the available screen space; longer content remains scrollable.

### Playlist limitations

Video format fetching accepts single videos and rejects playlist-only URLs. When a video URL also contains playlist parameters, Video+Audio fetches formats and downloads only that video.

Audio-only mode passes the URL to yt-dlp and processes every playlist item as a separate MP3. This behavior was verified in simulation against a two-item YouTube playlist. The app displays a reminder when Audio Only is selected, but it does not provide playlist selection or a queue interface.

## Development checks

From the repository root:

```sh
cargo test --locked --lib
node --test tests/*.test.cjs
node --check frontend/main.js
git diff --check
```

Rust tests include a simulated downloader and cancellation of its child process on Unix. Restricted environments may block the process inspection needed for that test. These tests do not download real media. Use `--offline` with Cargo when dependencies are already cached.

## Distribution status

The app targets macOS and Linux. The GUI has been tested interactively on macOS, and the x86-64 AppImage has been built on Ubuntu and manually verified on Linux. Its bundled-library audit verified 178 libraries with none unresolved. Windows packages are not planned. Release notes should identify the platforms tested and how each installer is signed.

Download the current packages from [YT-DLP Rusty GUI 0.2.1](https://github.com/KwisatzJim/ytdlprustygui-tauri/releases/tag/v0.2.1).

See [macOS release verification and installation](docs/MACOS_RELEASE.md) for the local Apple-silicon candidate and its signing status.

See [Linux release preparation](docs/LINUX_RELEASE.md) for the Ubuntu-server build and verification process.

### Linux AppImage troubleshooting

If AppImage bundling fails with `Strip call failed` and an unsupported `.relr.dyn` section, try skipping linuxdeploy's strip step:

```sh
NO_STRIP=true cargo tauri build -- --locked
```

This is a workaround for that packaging error, not a substitute for testing the resulting AppImage on the intended distributions.

## License

[MIT](LICENSE), copyright 2026 Jim Kelley. External tools retain their own licenses.
