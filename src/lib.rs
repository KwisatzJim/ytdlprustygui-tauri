use serde::{Deserialize, Serialize};
use std::fs;
use tauri::Manager;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AppConfig {
    #[serde(default)]
    output_dir: String,
    /// BCP-47-ish language code (e.g. "en", "es", "ja") used to auto-pick an
    /// audio track on videos that offer more than one language. `None`/absent
    /// means no preference — falls back to whatever the video creator marked
    /// as the default/original track.
    #[serde(default)]
    preferred_audio_language: Option<String>,
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
async fn fetch_formats(url: String) -> Result<FormatsResult, String> {
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
    let output = tokio::process::Command::new("yt-dlp")
        .strip_appimage_env()
        .args(["--no-warnings", "--no-playlist", "-J", &url])
        .output()
        .await
        .map_err(|e| format!("Failed to execute yt-dlp: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to fetch formats: {}",
            String::from_utf8_lossy(&output.stderr)
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

#[tauri::command]
async fn download(
    url: String,
    output_dir: String,
    download_type: String, // "video_audio" | "audio_only"
    video_format: Option<String>,
    audio_format: Option<String>,
) -> Result<(), String> {
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

    match download_type.as_str() {
        "video_audio" => {
            let vf = video_format.unwrap_or_default();
            let af = audio_format.unwrap_or_default();
            if vf.is_empty() || af.is_empty() {
                return Err("Please fetch and select both video and audio formats".into());
            }
            cmd.args([
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
                "-x",
                "--audio-format",
                "mp3",
                "-o",
                &format!("{output_dir}/%(title)s.%(ext)s"),
                &url,
            ]);
        }
        other => return Err(format!("Unknown download type: {other}")),
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to execute yt-dlp: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Download failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// GUI apps on macOS are launched by launchd, not by the user's shell, so they
/// inherit a minimal PATH (e.g. /usr/bin:/bin:/usr/sbin:/sbin) that doesn't
/// include Homebrew's /opt/homebrew/bin. This runs the user's actual login
/// shell once to capture their real PATH and applies it to this process, so
/// `Command::new("yt-dlp")` can find it the same way a terminal would.
#[cfg(target_os = "macos")]
fn fix_path_env() {
    use std::process::Command;

    const MARKER: &str = "__RUSTYGUI_PATH_MARKER__";
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());

    let output = Command::new(&shell)
        .args(["-ilc", &format!("printf {MARKER}; printf %s \"$PATH\"")])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(idx) = stdout.find(MARKER) {
                let path = stdout[idx + MARKER.len()..].trim();
                if !path.is_empty() {
                    // SAFETY: called once, single-threaded, before the Tauri
                    // runtime and any of its threads have started.
                    unsafe {
                        std::env::set_var("PATH", path);
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn fix_path_env() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    fix_path_env();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            check_ytdlp,
            fetch_formats,
            download,
            load_config,
            save_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
