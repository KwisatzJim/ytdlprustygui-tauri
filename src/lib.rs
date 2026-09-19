use serde::{Deserialize, Serialize};
use std::fs;
use tauri::Manager;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum AudioQuality {
    #[default]
    High,
    Low,
}

impl AudioQuality {
    fn mp3_quality(self) -> &'static str {
        match self {
            Self::High => "0",
            Self::Low => "9",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum SubtitleSource {
    Creator,
    Automatic,
    Both,
}

#[derive(Debug, Clone, Deserialize)]
struct SubtitleOptions {
    enabled: bool,
    source: SubtitleSource,
    languages: String,
    embed: bool,
}

fn subtitle_args(options: Option<&SubtitleOptions>) -> Result<Vec<String>, String> {
    let Some(options) = options.filter(|options| options.enabled) else {
        return Ok(Vec::new());
    };
    let languages = options.languages.trim();
    if languages.is_empty() || languages.len() > 200 || languages.contains(['\n', '\r', '\0']) {
        return Err("Enter valid subtitle languages, such as en.* or en.*,es".into());
    }
    let mut args = Vec::new();
    match options.source {
        SubtitleSource::Creator => args.push("--write-subs".into()),
        SubtitleSource::Automatic => args.push("--write-auto-subs".into()),
        SubtitleSource::Both => args.extend(["--write-subs".into(), "--write-auto-subs".into()]),
    }
    args.extend([
        "--sub-langs".into(), languages.into(),
        "--sub-format".into(), "srt/best".into(),
        "--convert-subs".into(), "srt".into(),
    ]);
    if options.embed {
        args.push("--embed-subs".into());
    }
    Ok(args)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MediaExtras {
    embed_thumbnail: bool,
    save_thumbnail: bool,
    embed_metadata: bool,
}

fn media_extra_args(options: Option<&MediaExtras>) -> Vec<&'static str> {
    let Some(options) = options else { return Vec::new(); };
    let mut args = Vec::new();
    if options.embed_thumbnail { args.push("--embed-thumbnail"); }
    if options.save_thumbnail { args.push("--write-thumbnail"); }
    if options.embed_metadata { args.push("--embed-metadata"); }
    args
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CookieSource {
    Browser,
    File,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CookieBrowser {
    Safari,
    Chrome,
    Chromium,
    Firefox,
    Brave,
    Edge,
    Opera,
    Vivaldi,
}

impl CookieBrowser {
    fn yt_dlp_name(self) -> &'static str {
        match self {
            Self::Safari => "safari",
            Self::Chrome => "chrome",
            Self::Chromium => "chromium",
            Self::Firefox => "firefox",
            Self::Brave => "brave",
            Self::Edge => "edge",
            Self::Opera => "opera",
            Self::Vivaldi => "vivaldi",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CookieOptions {
    source: CookieSource,
    browser: Option<CookieBrowser>,
    path: Option<String>,
}

fn cookie_args(options: Option<&CookieOptions>) -> Result<Vec<String>, String> {
    let Some(options) = options else { return Ok(Vec::new()); };
    match options.source {
        CookieSource::Browser => {
            let browser = options.browser.ok_or("Choose a browser for cookies")?;
            Ok(vec!["--cookies-from-browser".into(), browser.yt_dlp_name().into()])
        }
        CookieSource::File => {
            let path = options.path.as_deref().unwrap_or("").trim();
            let metadata = fs::metadata(path)
                .map_err(|error| format!("The selected cookie file is unavailable: {error}"))?;
            if !metadata.is_file() { return Err("The selected cookie path is not a file".into()); }
            use std::io::BufRead;
            let file = fs::File::open(path)
                .map_err(|error| format!("Could not read the selected cookie file: {error}"))?;
            let mut header = String::new();
            std::io::BufReader::new(file).read_line(&mut header)
                .map_err(|error| format!("Could not read the selected cookie file: {error}"))?;
            let header = header.trim_end_matches(['\r', '\n']);
            if header != "# HTTP Cookie File" && header != "# Netscape HTTP Cookie File" {
                return Err("The cookie file is not in Netscape format".into());
            }
            Ok(vec!["--cookies".into(), path.into()])
        }
    }
}

fn explain_cookie_error(error: String, options: Option<&CookieOptions>) -> String {
    let safari_permission_error = matches!(
        options,
        Some(CookieOptions {
            source: CookieSource::Browser,
            browser: Some(CookieBrowser::Safari),
            ..
        })
    ) && error.contains("Operation not permitted")
        && error.contains("Cookies.binarycookies");
    if safari_permission_error {
        return format!("{error}\nmacOS blocked access to Safari cookies. Use a Netscape cookie file or another browser, or grant Full Disk Access to YT-DLP Rusty GUI in System Settings → Privacy & Security and restart the app.");
    }
    if error.to_lowercase().contains("could not find") && error.to_lowercase().contains("cookies database") {
        let browser = options
            .and_then(|options| options.browser)
            .map(CookieBrowser::yt_dlp_name)
            .unwrap_or("selected browser");
        return format!("{error}\nThe {browser} profile could not be accessed. On macOS this may mean browser data is blocked by Privacy & Security even when the profile exists. Grant Full Disk Access to YT-DLP Rusty GUI and restart it, or select a Netscape cookie file.");
    }
    error
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AppConfig {
    #[serde(default)]
    theme: Theme,
    #[serde(default)]
    output_dir: String,
    /// BCP-47-ish language code (e.g. "en", "es", "ja") used to auto-pick an
    /// audio track on videos that offer more than one language. `None`/absent
    /// means no preference — falls back to whatever the video creator marked
    /// as the default/original track.
    #[serde(default)]
    preferred_audio_language: Option<String>,
    #[serde(default)]
    preferred_video_resolution: Option<u32>,
    #[serde(default)]
    preferred_audio_quality: AudioQuality,
    #[serde(default = "default_true")]
    automatic_ytdlp_updates: bool,
    #[serde(default)]
    last_ytdlp_update_check: Option<u64>,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

fn config_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to resolve config directory: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config directory: {e}"))?;
    Ok(dir.join("config.json"))
}

#[tauri::command]
fn load_config(app: tauri::AppHandle) -> Result<AppConfig, String> {
    let path = config_path(&app)?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let data = fs::read_to_string(&path).map_err(|e| format!("Failed to read config: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse config: {e}"))
}

#[tauri::command]
fn save_config(app: tauri::AppHandle, config: AppConfig) -> Result<(), String> {
    let path = config_path(&app)?;
    let data = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;
    fs::write(&path, data).map_err(|e| format!("Failed to write config: {e}"))
}

/// AppImages set PYTHONHOME/PYTHONPATH/LD_LIBRARY_PATH (and a few others) so
/// the bundled app can find its own libraries, but these leak into any child
/// process spawned from inside the AppImage. yt-dlp's own Python then reads a
/// PYTHONHOME pointing at the AppImage's mount, which has no stdlib, causing
/// "Failed to import encodings module". Stripping these on the child process
/// (not our own env) fixes it without affecting anything else.
trait StripAppimageEnv {
    fn strip_appimage_env(&mut self) -> &mut Self;
}

macro_rules! impl_strip_appimage_env {
    ($ty:ty) => {
        impl StripAppimageEnv for $ty {
            fn strip_appimage_env(&mut self) -> &mut Self {
                #[cfg(target_os = "linux")]
                {
                    self.env_remove("APPDIR")
                        .env_remove("APPIMAGE")
                        .env_remove("PYTHONHOME")
                        .env_remove("PYTHONPATH")
                        .env_remove("LD_LIBRARY_PATH");
                }
                self
            }
        }
    };
}

impl_strip_appimage_env!(std::process::Command);
impl_strip_appimage_env!(tokio::process::Command);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Format {
    id: String,
    extension: String,
    resolution: String,
    description: String,
    is_video: bool,
    is_audio: bool,
    /// Audio track language code (e.g. "en", "es"), when yt-dlp's extractor
    /// reports one. `None` for extractors/videos that don't expose per-track
    /// language (most sites only have one audio track anyway).
    language: Option<String>,
    /// True when the video creator marked this as the default/original audio
    /// track for its language (YouTube's multi-audio-track videos only).
    is_default_audio: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FormatsResult {
    video: Vec<Format>,
    audio: Vec<Format>,
}

#[derive(Debug, Clone, Serialize)]
struct DependencyStatus {
    name: &'static str,
    available: bool,
    detail: String,
}

fn dependency_detail(stdout: &[u8], stderr: &[u8]) -> String {
    let output = if stdout.is_empty() { stderr } else { stdout };
    String::from_utf8_lossy(output)
        .lines()
        .next()
        .unwrap_or("version unavailable")
        .trim()
        .chars()
        .take(160)
        .collect()
}

fn dependency_version_arg(executable: &str) -> &'static str {
    if executable == "yt-dlp" { "--version" } else { "-version" }
}

async fn dependency_status(name: &'static str, executable: &'static str) -> DependencyStatus {
    let mut command = tokio::process::Command::new(executable);
    command.strip_appimage_env().arg(dependency_version_arg(executable));
    match tokio::time::timeout(std::time::Duration::from_secs(5), command.output()).await {
        Ok(Ok(output)) if output.status.success() => DependencyStatus {
            name,
            available: true,
            detail: dependency_detail(&output.stdout, &output.stderr),
        },
        Ok(Ok(output)) => DependencyStatus {
            name,
            available: false,
            detail: format!("version check failed ({})", output.status),
        },
        Ok(Err(_)) => DependencyStatus {
            name,
            available: false,
            detail: "not found on PATH".into(),
        },
        Err(_) => DependencyStatus {
            name,
            available: false,
            detail: "version check timed out".into(),
        },
    }
}

#[tauri::command]
async fn check_dependencies() -> Vec<DependencyStatus> {
    let (yt_dlp, ffmpeg, ffprobe) = tokio::join!(
        dependency_status("yt-dlp", "yt-dlp"),
        dependency_status("FFmpeg", "ffmpeg"),
        dependency_status("FFprobe", "ffprobe"),
    );
    vec![yt_dlp, ffmpeg, ffprobe]
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct YtDlpUpdateResult {
    previous_version: String,
    current_version: String,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum YtDlpUpdateMethod {
    Homebrew,
    Apt,
    SelfUpdate,
}

fn ytdlp_update_method(executable: &std::path::Path) -> YtDlpUpdateMethod {
    let path = executable.to_string_lossy();
    if path.contains("/Cellar/yt-dlp/") || path.contains("/homebrew/") {
        YtDlpUpdateMethod::Homebrew
    } else if path == "/usr/bin/yt-dlp" {
        YtDlpUpdateMethod::Apt
    } else {
        YtDlpUpdateMethod::SelfUpdate
    }
}

async fn command_output(mut command: tokio::process::Command) -> Result<std::process::Output, String> {
    command.strip_appimage_env();
    tokio::time::timeout(std::time::Duration::from_secs(300), command.output())
        .await
        .map_err(|_| "The yt-dlp update timed out after five minutes".to_string())?
        .map_err(|error| format!("Could not start the yt-dlp updater: {error}"))
}

fn update_message(output: &std::process::Output) -> String {
    let bytes = if output.stdout.is_empty() { &output.stderr } else { &output.stdout };
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(600)
        .collect()
}

#[tauri::command]
async fn update_ytdlp() -> Result<YtDlpUpdateResult, String> {
    let previous_version = check_ytdlp()?;
    let which_output = command_output({
        let mut command = tokio::process::Command::new("which");
        command.arg("yt-dlp");
        command
    }).await?;
    if !which_output.status.success() {
        return Err("Could not locate the active yt-dlp executable".into());
    }
    let executable = std::path::PathBuf::from(String::from_utf8_lossy(&which_output.stdout).trim());
    let method = ytdlp_update_method(&executable.canonicalize().unwrap_or(executable));
    if method == YtDlpUpdateMethod::Apt {
        return Err("This yt-dlp installation is managed by Ubuntu/Debian apt. Update it with your normal system updates: sudo apt update && sudo apt install --only-upgrade yt-dlp".into());
    }

    let output = match method {
        YtDlpUpdateMethod::Homebrew => command_output({
            let mut command = tokio::process::Command::new("brew");
            command.args(["upgrade", "yt-dlp"]);
            command
        }).await?,
        YtDlpUpdateMethod::SelfUpdate => command_output({
            let mut command = tokio::process::Command::new("yt-dlp");
            command.arg("-U");
            command
        }).await?,
        YtDlpUpdateMethod::Apt => unreachable!(),
    };
    let message = update_message(&output);
    if !output.status.success() {
        return Err(format!("yt-dlp update failed: {}", if message.is_empty() { output.status.to_string() } else { message }));
    }
    let current_version = check_ytdlp()?;
    Ok(YtDlpUpdateResult {
        previous_version,
        current_version,
        message: if message.is_empty() { "Update check completed".into() } else { message },
    })
}

fn existing_folder(path: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(path.trim());
    if path.as_os_str().is_empty() {
        return Err("No download folder is available to open".into());
    }
    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_dir() => Ok(path),
        Ok(_) => Err("The download location is not a folder".into()),
        Err(error) => Err(format!("The download folder is unavailable: {error}")),
    }
}

#[tauri::command]
async fn open_output_folder(path: String) -> Result<(), String> {
    let path = existing_folder(&path)?;
    #[cfg(target_os = "macos")]
    let mut command = tokio::process::Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = tokio::process::Command::new("xdg-open");
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return Err("Opening the download folder is unsupported on this platform".into());

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        command.strip_appimage_env().arg(path);
        let status = tokio::time::timeout(std::time::Duration::from_secs(5), command.status())
            .await
            .map_err(|_| "Opening the download folder timed out".to_string())?
            .map_err(|error| format!("Could not open the download folder: {error}"))?;
        if status.success() { Ok(()) } else { Err(format!("Could not open the download folder ({status})")) }
    }
}

/// Checks that yt-dlp is installed and reachable on PATH. Called by the
/// frontend on startup so we can show a clear error instead of failing
/// silently on the first download attempt.
#[tauri::command]
fn check_ytdlp() -> Result<String, String> {
    match std::process::Command::new("yt-dlp")
        .strip_appimage_env()
        .arg("--version")
        .output()
    {
        Ok(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        }
        Ok(output) => Err(format!(
            "yt-dlp returned an error: {}",
            String::from_utf8_lossy(&output.stderr)
        )),
        Err(_) => Err(
            "yt-dlp not found. Please install yt-dlp and make sure it's on your PATH.".into(),
        ),
    }
}

/// Raw per-format fields we care about from yt-dlp's JSON output. `#[serde(default)]`
/// on everything but `format_id`/`ext` because not every extractor populates every
/// field, and a missing field should mean "unknown", not a parse failure.
#[derive(Debug, Clone, Deserialize)]
struct YtDlpFormat {
    format_id: String,
    ext: String,
    #[serde(default)]
    resolution: Option<String>,
    #[serde(default)]
    vcodec: Option<String>,
    #[serde(default)]
    acodec: Option<String>,
    #[serde(default)]
    width: Option<f64>,
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    fps: Option<f64>,
    #[serde(default)]
    abr: Option<f64>,
    #[serde(default)]
    vbr: Option<f64>,
    #[serde(default)]
    tbr: Option<f64>,
    #[serde(default)]
    filesize: Option<f64>,
    #[serde(default)]
    filesize_approx: Option<f64>,
    #[serde(default)]
    format_note: Option<String>,
    #[serde(default)]
    language: Option<String>,
    /// YouTube-specific: 10 = the original-language track, 5 = a track the
    /// creator otherwise marked default, lower/absent = a dubbed/alternate
    /// track. Other extractors leave this unset.
    #[serde(default)]
    language_preference: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct YtDlpInfo {
    #[serde(default)]
    formats: Option<Vec<YtDlpFormat>>,
    #[serde(default)]
    entries: Option<Vec<serde_json::Value>>,
}

fn human_size(bytes: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.2}{}", UNITS[unit])
}

fn build_description(f: &YtDlpFormat, is_video: bool, is_audio: bool) -> String {
    let mut parts: Vec<String> = Vec::new();

    if is_video {
        if let Some(vcodec) = f.vcodec.as_deref().filter(|v| *v != "none") {
            parts.push(vcodec.to_string());
        }
        if let Some(vbr) = f.vbr {
            parts.push(format!("{}k video", vbr.round() as i64));
        }
        if let Some(fps) = f.fps {
            parts.push(format!("{}fps", fps.round() as i64));
        }
    }

    if is_audio {
        if let Some(acodec) = f.acodec.as_deref().filter(|a| *a != "none") {
            parts.push(acodec.to_string());
        }
        if let Some(abr) = f.abr {
            parts.push(format!("{}k audio", abr.round() as i64));
        }
    }

    if let Some(size) = f.filesize.or(f.filesize_approx) {
        parts.push(human_size(size));
    } else if let Some(tbr) = f.tbr {
        parts.push(format!("~{}k", tbr.round() as i64));
    }

    if let Some(note) = f.format_note.as_deref().filter(|n| !n.is_empty()) {
        parts.push(note.to_string());
    }

    parts.join(", ")
}

#[tauri::command]
async fn fetch_formats(url: String, cookies: Option<CookieOptions>) -> Result<FormatsResult, String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("Please enter a URL first".into());
    }

    // Structured JSON instead of scraping the human-readable --list-formats
    // table: that table's column layout (which columns appear, multi-word
    // cells like "audio only", the │/- delimiter glyphs) varies per video and
    // per yt-dlp version, which made the old whitespace-splitting parser
    // silently misclassify or drop formats on some videos. -J/--dump-single-json
    // gives every format's fields directly, so there's no table to misparse.
    // --no-playlist keeps this to the single video at the URL, matching what
    // the rest of this app (and the old table parser) already assumed.
    let mut command = tokio::process::Command::new("yt-dlp");
    command.strip_appimage_env();
    command.args(cookie_args(cookies.as_ref())?);
    let output = command
        .args(["--no-warnings", "--no-playlist", "-J", &url])
        .output()
        .await
        .map_err(|e| format!("Failed to execute yt-dlp: {e}"))?;

    if !output.status.success() {
        return Err(explain_cookie_error(
            format!("Failed to fetch formats: {}", String::from_utf8_lossy(&output.stderr)),
            cookies.as_ref(),
        ));
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let info: YtDlpInfo = serde_json::from_str(&output_str)
        .map_err(|e| format!("Failed to parse yt-dlp's output as JSON: {e}"))?;

    if info.entries.is_some() {
        return Err("This URL is a playlist. Please paste a single video URL.".into());
    }

    let raw_formats = info
        .formats
        .ok_or_else(|| "yt-dlp did not report any formats for this URL".to_string())?;

    let mut video = Vec::new();
    let mut audio = Vec::new();

    for f in raw_formats {
        // Storyboard "formats" (mhtml thumbnail sheets) aren't real media.
        if f.ext == "mhtml" {
            continue;
        }

        let is_video = f.vcodec.as_deref().map(|v| v != "none").unwrap_or(false);
        let is_audio = f.acodec.as_deref().map(|a| a != "none").unwrap_or(false);
        if !is_video && !is_audio {
            continue;
        }

        let resolution = f.resolution.clone().unwrap_or_else(|| match (f.width, f.height) {
            (Some(w), Some(h)) => format!("{}x{}", w as i64, h as i64),
            _ if is_video => "unknown".to_string(),
            _ => "audio only".to_string(),
        });

        let mut description = build_description(&f, is_video, is_audio);
        // language_preference >= 5 is yt-dlp/YouTube's way of marking a track
        // as the creator's chosen default/original for its language.
        let is_default_audio = f.language_preference.map(|p| p >= 5).unwrap_or(false);
        if is_audio {
            if let Some(lang) = f.language.as_deref().filter(|l| !l.is_empty()) {
                description.push_str(&format!(", [{lang}]"));
            }
            if is_default_audio {
                description.push_str(" (default)");
            }
        }

        let format = Format {
            id: f.format_id.clone(),
            extension: f.ext.clone(),
            resolution,
            description,
            is_video,
            is_audio,
            language: f.language.clone(),
            is_default_audio,
        };

        if is_video {
            video.push(format.clone());
        }
        if is_audio {
            audio.push(format);
        }
    }

    if video.is_empty() && audio.is_empty() {
        return Err("No formats available or could not distinguish audio/video formats".into());
    }

    Ok(FormatsResult { video, audio })
}

// Drain both pipes while yt-dlp runs, retaining only a short diagnostic tail.
// Progress can be written to either pipe, depending on yt-dlp's configuration.
async fn read_download_output(
    reader: impl AsyncRead + Unpin,
    on_progress: &impl Fn(String),
) -> Result<String, String> {
    let mut lines = BufReader::new(reader).split(b'\n');
    let mut tail = std::collections::VecDeque::new();
    while let Some(bytes) = lines.next_segment().await
        .map_err(|e| format!("Failed to read download output: {e}"))? {
        let line = String::from_utf8_lossy(&bytes).trim().to_string();
        if line.is_empty() { continue; }
        if ["[download]", "[Merger]", "[ExtractAudio]", "[VideoRemuxer]", "[Fixup]", "[Metadata]", "[EmbedThumbnail]"]
            .iter().any(|prefix| line.starts_with(prefix)) {
            on_progress(line.chars().take(2000).collect());
        }
        tail.push_back(line.chars().take(2000).collect::<String>());
        if tail.len() > 20 { tail.pop_front(); }
    }
    Ok(tail.into_iter().collect::<Vec<_>>().join("\n"))
}

#[derive(Default)]
struct DownloadState(std::sync::Mutex<Option<ActiveDownload>>);

struct ActiveDownload {
    id: String,
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
}

// Release the active slot on success, cancellation, or any error.
struct DownloadGuard<'a>(&'a DownloadState);

impl Drop for DownloadGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.0.0.lock() { *active = None; }
    }
}

impl DownloadState {
    fn begin(&self, id: String) -> Result<(DownloadGuard<'_>, tokio::sync::oneshot::Receiver<()>), String> {
        let mut active = self.0.lock().map_err(|_| "Download state is unavailable")?;
        if active.is_some() { return Err("A download is already running".into()); }
        let (sender, receiver) = tokio::sync::oneshot::channel();
        *active = Some(ActiveDownload { id, cancel: Some(sender) });
        Ok((DownloadGuard(self), receiver))
    }

    fn cancel(&self, id: &str) -> Result<bool, String> {
        let mut active = self.0.lock().map_err(|_| "Download state is unavailable")?;
        let Some(download) = active.as_mut().filter(|download| download.id == id) else {
            return Ok(false);
        };
        if let Some(sender) = download.cancel.take() { let _ = sender.send(()); }
        Ok(true)
    }
}

#[tauri::command]
fn cancel_download(download_id: String, downloads: tauri::State<'_, DownloadState>) -> Result<bool, String> {
    downloads.cancel(&download_id)
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum DownloadOutcome { Completed, Cancelled }

async fn stop_download_tree(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        let group = i32::try_from(pid).map_err(|_| "Invalid download process ID")?;
        if group <= 1 { return Err("Invalid download process group".into()); }
        // SAFETY: this is the positive PID returned by spawn for our own child,
        // made leader of a separate group below. A negative PID targets that group.
        if unsafe { libc::kill(-group, libc::SIGKILL) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(format!("Could not stop download: {error}"));
            }
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        let system_root = std::env::var_os("SystemRoot").ok_or("Windows system directory is unavailable")?;
        let output = tokio::process::Command::new(std::path::PathBuf::from(system_root).join("System32/taskkill.exe"))
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output().await.map_err(|e| format!("Could not stop download: {e}"))?;
        if output.status.success() { Ok(()) } else {
            Err(format!("Could not stop download: {}", String::from_utf8_lossy(&output.stderr)))
        }
    }
    #[cfg(not(any(unix, windows)))]
    { let _ = pid; Err("Cancellation is not supported on this platform".into()) }
}

async fn run_download(
    mut cmd: tokio::process::Command,
    on_progress: impl Fn(String),
    cancel: tokio::sync::oneshot::Receiver<()>,
) -> Result<DownloadOutcome, String> {
    #[cfg(unix)]
    cmd.process_group(0);
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Failed to execute yt-dlp: {e}"))?;
    let pid = child.id().ok_or("Download process ID is unavailable")?;
    on_progress("Starting download...".into());
    let stdout = child.stdout.take().ok_or("Download output pipe is unavailable")?;
    let stderr = child.stderr.take().ok_or("Download error pipe is unavailable")?;
    let finished = tokio::select! {
        biased;
        output = async { tokio::join!(
            read_download_output(stdout, &on_progress),
            read_download_output(stderr, &on_progress),
            child.wait(),
        ) } => Some(output),
        _ = async {
            // Dropping a sender is not a request to cancel.
            if cancel.await.is_err() { std::future::pending::<()>().await; }
        } => None,
    };
    let Some((stdout, stderr, status)) = finished else {
        stop_download_tree(pid).await?;
        child.wait().await.map_err(|e| format!("Failed to finish cancellation: {e}"))?;
        return Ok(DownloadOutcome::Cancelled);
    };
    let status = status.map_err(|e| format!("Failed to wait for yt-dlp: {e}"))?;
    let stdout = stdout?;
    let stderr = stderr?;
    if status.success() {
        Ok(DownloadOutcome::Completed)
    } else {
        let details = if stderr.is_empty() { stdout } else { stderr };
        Err(format!("Download failed ({status}): {details}"))
    }
}

async fn check_media_tool(name: &str, mut command: tokio::process::Command) -> Result<(), String> {
    command.strip_appimage_env().arg("-version").kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(5), command.output())
        .await.map_err(|_| format!("{name} did not respond within 5 seconds. Check its installation and try again."))?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("{name} was not found. Install FFmpeg (including ffprobe) and make sure both programs are on your PATH, then try again.")
            } else {
                format!("Could not start {name}: {e}. Check its installation and permissions.")
            }
        })?;
    if !output.status.success() {
        let details = String::from_utf8_lossy(&output.stderr).chars().take(1000).collect::<String>();
        return Err(format!("{name} failed its version check ({}). Check its installation. {details}", output.status));
    }
    Ok(())
}

async fn check_media_tools() -> Result<(), String> {
    let (ffmpeg, ffprobe) = tokio::join!(
        check_media_tool("FFmpeg", tokio::process::Command::new("ffmpeg")),
        check_media_tool("FFprobe", tokio::process::Command::new("ffprobe")),
    );
    let errors: Vec<_> = [ffmpeg, ffprobe].into_iter().filter_map(Result::err).collect();
    if errors.is_empty() { Ok(()) } else { Err(errors.join("\n")) }
}

#[tauri::command]
async fn download(
    url: String,
    output_dir: String,
    download_type: String, // "video_audio" | "audio_only"
    video_format: Option<String>,
    audio_format: Option<String>,
    audio_quality: Option<AudioQuality>,
    subtitles: Option<SubtitleOptions>,
    media_extras: Option<MediaExtras>,
    cookies: Option<CookieOptions>,
    on_progress: tauri::ipc::Channel<String>,
    download_id: String,
    downloads: tauri::State<'_, DownloadState>,
) -> Result<DownloadOutcome, String> {
    let url = url.trim().to_string();
    let output_dir = output_dir.trim().to_string();

    if url.is_empty() {
        return Err("Please enter a URL first".into());
    }
    if output_dir.is_empty() {
        return Err("Please select an output directory".into());
    }

    let mut cmd = tokio::process::Command::new("yt-dlp");
    cmd.strip_appimage_env();
    cmd.args(["--newline", "--progress", "--no-colors"]);
    cmd.args(media_extra_args(media_extras.as_ref()));
    cmd.args(cookie_args(cookies.as_ref())?);

    match download_type.as_str() {
        "video_audio" => {
            let vf = video_format.unwrap_or_default();
            let af = audio_format.unwrap_or_default();
            if vf.is_empty() || af.is_empty() {
                return Err("Please fetch and select both video and audio formats".into());
            }
            cmd.args(subtitle_args(subtitles.as_ref())?);
            cmd.args([
                // Match fetch_formats: these format IDs belong to one video.
                "--no-playlist",
                "-f",
                &format!("{vf}+{af}"),
                "-o",
                &format!("{output_dir}/%(title)s.%(ext)s"),
                "--merge-output-format",
                "mp4",
                &url,
            ]);
        }
        "audio_only" => {
            cmd.args([
                "-f",
                "bestaudio/best",
                "-x",
                "--audio-format",
                "mp3",
                "--audio-quality",
                audio_quality.unwrap_or_default().mp3_quality(),
                "-o",
                &format!("{output_dir}/%(title)s.%(ext)s"),
                &url,
            ]);
        }
        other => return Err(format!("Unknown download type: {other}")),
    }

    check_media_tools().await?;
    let (_guard, cancel) = downloads.begin(download_id)?;
    run_download(cmd, |message| { let _ = on_progress.send(message); }, cancel)
        .await
        .map_err(|error| explain_cookie_error(error, cookies.as_ref()))
}

fn path_after_marker(output: &[u8], marker: &str) -> Option<String> {
    let stdout = String::from_utf8_lossy(output);
    let path = stdout.split(marker).nth(1)?.trim();
    (!path.is_empty()).then(|| path.to_string())
}

/// Apps opened from a macOS or Linux desktop may not inherit the PATH created
/// by the user's shell setup. Ask their login shell for it once so Homebrew and
/// other user-installed tools are found just as they are in a terminal.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn fix_path_env() {
    use std::process::Command;

    const MARKER: &str = "__RUSTYGUI_PATH_MARKER__";
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());

    let output = Command::new(&shell)
        .args(["-ilc", &format!("printf {MARKER}; printf %s \"$PATH\"")])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            if let Some(path) = path_after_marker(&output.stdout, MARKER) {
                // SAFETY: called once, single-threaded, before the Tauri
                // runtime and any of its threads have started.
                unsafe {
                    std::env::set_var("PATH", path);
                }
            }
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn fix_path_env() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    fix_path_env();

    tauri::Builder::default()
        .manage(DownloadState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            check_ytdlp,
            check_dependencies,
            update_ytdlp,
            open_output_folder,
            fetch_formats,
            download,
            cancel_download,
            load_config,
            save_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, AudioQuality, Theme};

    #[test]
    fn login_shell_path_ignores_startup_noise() {
        let output = b"shell greeting\n__RUSTYGUI_PATH_MARKER__/home/linuxbrew/.linuxbrew/bin:/usr/bin\n";
        assert_eq!(
            super::path_after_marker(output, "__RUSTYGUI_PATH_MARKER__").as_deref(),
            Some("/home/linuxbrew/.linuxbrew/bin:/usr/bin")
        );
        assert!(super::path_after_marker(b"no marker", "marker").is_none());
        assert!(super::path_after_marker(b"marker   ", "marker").is_none());
    }

    #[test]
    fn dependency_version_uses_one_bounded_line() {
        assert_eq!(super::dependency_detail(b"2026.09.04\nextra\n", b""), "2026.09.04");
        assert_eq!(super::dependency_detail(b"", b"ffmpeg version 8.0\nmore"), "ffmpeg version 8.0");
        assert_eq!(super::dependency_version_arg("yt-dlp"), "--version");
        assert_eq!(super::dependency_version_arg("ffmpeg"), "-version");
        assert_eq!(super::dependency_version_arg("ffprobe"), "-version");
    }

    #[test]
    fn updater_uses_the_installation_owner() {
        use super::YtDlpUpdateMethod;
        assert_eq!(super::ytdlp_update_method(std::path::Path::new("/opt/homebrew/Cellar/yt-dlp/2026/bin/yt-dlp")), YtDlpUpdateMethod::Homebrew);
        assert_eq!(super::ytdlp_update_method(std::path::Path::new("/usr/bin/yt-dlp")), YtDlpUpdateMethod::Apt);
        assert_eq!(super::ytdlp_update_method(std::path::Path::new("/home/jim/.local/bin/yt-dlp")), YtDlpUpdateMethod::SelfUpdate);
    }

    #[test]
    fn open_folder_requires_an_existing_directory() {
        assert!(super::existing_folder("").is_err());
        assert!(super::existing_folder("Cargo.toml").unwrap_err().contains("not a folder"));
        assert_eq!(super::existing_folder(".").unwrap(), std::path::PathBuf::from("."));
    }

    #[tokio::test]
    async fn missing_media_tool_has_actionable_error() {
        let missing = std::env::temp_dir().join("rustygui-nonexistent-tools").join("ffmpeg-not-installed");
        let error = super::check_media_tool("FFmpeg", tokio::process::Command::new(missing)).await.unwrap_err();
        assert!(error.contains("FFmpeg was not found"));
        assert!(error.contains("PATH"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn media_tool_check_accepts_success_and_reports_failure() {
        let mut good = tokio::process::Command::new("/bin/sh");
        good.args(["-c", "exit 0"]);
        assert!(super::check_media_tool("FFmpeg", good).await.is_ok());
        let mut bad = tokio::process::Command::new("/bin/sh");
        bad.args(["-c", "printf 'broken library' >&2; exit 1"]);
        let error = super::check_media_tool("FFprobe", bad).await.unwrap_err();
        assert!(error.contains("FFprobe failed its version check"));
        assert!(error.contains("broken library"));
    }

    #[test]
    fn cancellation_targets_only_the_current_download_and_releases_slot() {
        let state = super::DownloadState::default();
        let (guard, mut receiver) = state.begin("first".into()).unwrap();
        assert!(state.begin("second".into()).is_err());
        assert!(!state.cancel("old").unwrap());
        assert!(receiver.try_recv().is_err());
        assert!(state.cancel("first").unwrap());
        assert!(receiver.try_recv().is_ok());
        assert!(state.cancel("first").unwrap());
        drop(guard);
        let (_guard, mut receiver) = state.begin("second".into()).unwrap();
        assert!(!state.cancel("first").unwrap());
        assert!(receiver.try_recv().is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_stops_the_downloader_and_its_child() {
        let mut cmd = tokio::process::Command::new("/bin/sh");
        cmd.args(["-c", "sleep 30 & child=$!; printf '[download] child:%s\\n' \"$child\"; wait"]);
        let (sender, cancel) = tokio::sync::oneshot::channel();
        let sender = std::sync::Mutex::new(Some(sender));
        let child_pid = std::sync::Mutex::new(None);
        let running = super::run_download(cmd, |line| {
            if let Some(pid) = line.strip_prefix("[download] child:") {
                *child_pid.lock().unwrap() = Some(pid.parse::<u32>().unwrap());
                sender.lock().unwrap().take().unwrap().send(()).unwrap();
            }
        }, cancel);
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(5), running)
            .await.expect("cancellation should finish promptly").unwrap();
        assert_eq!(outcome, super::DownloadOutcome::Cancelled);
        let pid = child_pid.lock().unwrap().unwrap();
        // A killed orphan can briefly remain as a zombie until the OS reaps it.
        let stopped = async {
            loop {
                let status = tokio::process::Command::new("/bin/ps")
                    .args(["-p", &pid.to_string(), "-o", "stat="]).output().await.unwrap();
                let state = String::from_utf8_lossy(&status.stdout);
                if !status.status.success() || state.trim().starts_with('Z') { break; }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        };
        tokio::time::timeout(std::time::Duration::from_secs(2), stopped)
            .await.expect("conversion child must also stop");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn ordinary_completion_is_not_cancellation() {
        let mut cmd = tokio::process::Command::new("/bin/sh");
        cmd.args(["-c", "exit 0"]);
        let (_sender, receiver) = tokio::sync::oneshot::channel();
        assert_eq!(super::run_download(cmd, |_| {}, receiver).await.unwrap(), super::DownloadOutcome::Completed);
    }

    #[tokio::test]
    async fn progress_arrives_before_output_closes() {
        use tokio::io::AsyncWriteExt;
        let (mut writer, reader) = tokio::io::duplex(256);
        let (sent, received) = tokio::sync::oneshot::channel();
        let sent = std::cell::RefCell::new(Some(sent));
        let callback = |line: String| {
            assert_eq!(line, "[download] 25% at 2MiB/s");
            sent.borrow_mut().take().unwrap().send(()).unwrap();
        };
        let producer = async {
            writer.write_all(b"[download] 25% at 2MiB/s\n").await.unwrap();
            // The reader must send progress before EOF, not buffer until exit.
            received.await.unwrap();
            writer.shutdown().await.unwrap();
        };
        let (output, _) = tokio::join!(super::read_download_output(reader, &callback), producer);
        assert!(output.unwrap().contains("25%"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn downloader_drains_both_pipes_and_preserves_failure() {
        let mut cmd = tokio::process::Command::new("/bin/sh");
        cmd.args(["-c", "printf '[download] 50%%\\n'; printf '[ExtractAudio] converting\\nERROR: conversion failed\\n' >&2; exit 1"]);
        let progress = std::sync::Mutex::new(Vec::new());
        let (_sender, cancel) = tokio::sync::oneshot::channel();
        let result = super::run_download(cmd, |line| progress.lock().unwrap().push(line), cancel).await;
        assert!(result.unwrap_err().contains("conversion failed"));
        let messages = progress.lock().unwrap();
        assert!(messages.iter().any(|line| line.contains("50%")));
        assert!(messages.iter().any(|line| line.contains("ExtractAudio")));
    }

    #[tokio::test]
    async fn diagnostics_are_bounded_and_non_utf8_is_tolerated() {
        let mut data = (0..100).map(|i| format!("line {i}\n")).collect::<String>().into_bytes();
        data.extend_from_slice(b"invalid: \xff\n");
        let output = super::read_download_output(data.as_slice(), &|_| {}).await.unwrap();
        assert_eq!(output.lines().count(), 20);
        assert!(output.contains("line 99"));
        assert!(output.contains("invalid:"));
    }

    #[test]
    fn audio_quality_defaults_and_persistence() {
        let mut config: AppConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config.preferred_audio_quality, AudioQuality::High);
        assert!(config.automatic_ytdlp_updates);
        assert_eq!(config.last_ytdlp_update_check, None);
        config.preferred_audio_quality = AudioQuality::Low;
        let restored: AppConfig = serde_json::from_str(
            &serde_json::to_string(&config).unwrap(),
        ).unwrap();
        assert_eq!(restored.preferred_audio_quality, AudioQuality::Low);
        assert_eq!(AudioQuality::High.mp3_quality(), "0");
        assert_eq!(AudioQuality::Low.mp3_quality(), "9");
        assert!(serde_json::from_str::<AudioQuality>("\"invalid\"").is_err());
    }

    #[test]
    fn resolution_preference_is_backward_compatible_and_round_trips() {
        let mut config: AppConfig = serde_json::from_str(
            r#"{"output_dir":"/tmp/videos","preferred_audio_language":"en"}"#,
        ).unwrap();
        assert_eq!(config.preferred_video_resolution, None);
        assert_eq!(config.theme, Theme::System);
        config.theme = Theme::Dark;
        config.preferred_video_resolution = Some(1080);
        let saved = serde_json::to_string(&config).unwrap();
        let restored: AppConfig = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.preferred_video_resolution, Some(1080));
        assert_eq!(restored.theme, Theme::Dark);
        assert_eq!(restored.output_dir, "/tmp/videos");
        assert_eq!(restored.preferred_audio_language.as_deref(), Some("en"));
    }

    #[test]
    fn subtitle_options_build_safe_explicit_arguments() {
        let embedded = super::SubtitleOptions {
            enabled: true,
            source: super::SubtitleSource::Both,
            languages: "en.*,es".into(),
            embed: true,
        };
        let args = super::subtitle_args(Some(&embedded)).unwrap();
        assert!(args.contains(&"--write-subs".to_string()));
        assert!(args.contains(&"--write-auto-subs".to_string()));
        assert!(args.contains(&"--embed-subs".to_string()));
        assert!(!args.iter().any(|arg| arg == "--no-keep-subs"));
        assert!(args.windows(2).any(|pair| pair == ["--sub-langs", "en.*,es"]));

        let separate = super::SubtitleOptions {
            embed: false,
            source: super::SubtitleSource::Creator,
            ..embedded
        };
        let args = super::subtitle_args(Some(&separate)).unwrap();
        assert!(args.contains(&"--write-subs".to_string()));
        assert!(!args.contains(&"--write-auto-subs".to_string()));
        assert!(!args.contains(&"--embed-subs".to_string()));
        assert!(super::subtitle_args(None).unwrap().is_empty());
    }

    #[test]
    fn subtitle_languages_are_bounded_and_single_line() {
        let options = super::SubtitleOptions {
            enabled: true,
            source: super::SubtitleSource::Automatic,
            languages: "en\n--other-option".into(),
            embed: false,
        };
        assert!(super::subtitle_args(Some(&options)).is_err());
    }


    #[test]
    fn thumbnail_and_metadata_options_are_independent() {
        let options = super::MediaExtras {
            embed_thumbnail: true,
            save_thumbnail: false,
            embed_metadata: true,
        };
        assert_eq!(
            super::media_extra_args(Some(&options)),
            vec!["--embed-thumbnail", "--embed-metadata"]
        );
        let separate = super::MediaExtras {
            save_thumbnail: true,
            ..Default::default()
        };
        assert_eq!(super::media_extra_args(Some(&separate)), vec!["--write-thumbnail"]);
        assert!(super::media_extra_args(None).is_empty());
    }


    #[test]
    fn browser_cookie_sources_use_supported_fixed_names() {
        let options = super::CookieOptions {
            source: super::CookieSource::Browser,
            browser: Some(super::CookieBrowser::Firefox),
            path: None,
        };
        assert_eq!(
            super::cookie_args(Some(&options)).unwrap(),
            vec!["--cookies-from-browser", "firefox"]
        );
        assert!(super::cookie_args(None).unwrap().is_empty());
    }

    #[test]
    fn cookie_files_require_netscape_header() {
        let path = std::env::temp_dir().join(format!("rustygui-cookies-{}.txt", std::process::id()));
        std::fs::write(&path, "not a cookie jar\n").unwrap();
        let options = super::CookieOptions {
            source: super::CookieSource::File,
            browser: None,
            path: Some(path.to_string_lossy().into()),
        };
        assert!(super::cookie_args(Some(&options)).unwrap_err().contains("Netscape"));
        std::fs::write(&path, "# Netscape HTTP Cookie File\n").unwrap();
        assert_eq!(super::cookie_args(Some(&options)).unwrap()[0], "--cookies");
        std::fs::remove_file(path).unwrap();
    }


    #[test]
    fn safari_permission_errors_explain_macos_fix() {
        let options = super::CookieOptions {
            source: super::CookieSource::Browser,
            browser: Some(super::CookieBrowser::Safari),
            path: None,
        };
        let error = super::explain_cookie_error(
            "Operation not permitted: /Users/test/Library/Cookies/Cookies.binarycookies".into(),
            Some(&options),
        );
        assert!(error.contains("Full Disk Access"));
        assert!(error.contains("Netscape cookie file"));
    }


    #[test]
    fn missing_browser_profile_errors_explain_cookie_choices() {
        let options = super::CookieOptions {
            source: super::CookieSource::Browser,
            browser: Some(super::CookieBrowser::Firefox),
            path: None,
        };
        let error = super::explain_cookie_error(
            "could not find firefox cookies database".into(),
            Some(&options),
        );
        assert!(error.contains("firefox profile could not be accessed"));
        assert!(error.contains("Full Disk Access"));
        assert!(error.contains("Netscape cookie file"));
    }

}
