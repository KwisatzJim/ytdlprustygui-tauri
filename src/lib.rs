use serde::{Deserialize, Serialize};
use std::fs;
use tauri::Manager;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AppConfig {
    #[serde(default)]
    output_dir: String,
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

#[tauri::command]
async fn fetch_formats(url: String) -> Result<FormatsResult, String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("Please enter a URL first".into());
    }

    let output = tokio::process::Command::new("yt-dlp")
        .strip_appimage_env()
        .args(["--list-formats", &url])
        .output()
        .await
        .map_err(|e| format!("Failed to execute yt-dlp: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to fetch formats: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let output_str = String::from_utf8_lossy(&output.stdout).to_string();
    let formats = parse_formats(&output_str);

    let video: Vec<Format> = formats.iter().filter(|f| f.is_video).cloned().collect();
    let audio: Vec<Format> = formats.iter().filter(|f| f.is_audio).cloned().collect();

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

fn parse_formats(output: &str) -> Vec<Format> {
    let mut formats = Vec::new();

    // Flag to indicate we've reached the format table section
    let mut in_format_table = false;

    for line in output.lines() {
        // Skip lines until we find the format table header
        if line.contains("ID") && line.contains("EXT") && line.contains("RESOLUTION") {
            in_format_table = true;
            continue;
        }

        if !in_format_table {
            continue;
        }

        // Skip empty lines or lines without format information
        if line.trim().is_empty() || !line.contains(' ') {
            continue;
        }

        // Parse format line
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            // The format ID is always the first part
            let id = parts[0].to_string();

            // Extract extension (usually the second part)
            let extension = if parts.len() > 1 {
                parts[1].to_string()
            } else {
                "unknown".to_string()
            };

            // Extract resolution if available
            let resolution = if parts.len() > 2 && parts[2].contains('x') {
                parts[2].to_string()
            } else {
                "audio only".to_string()
            };

            // Join remaining parts as description
            let description = if parts.len() > 3 {
                parts[3..].join(" ")
            } else {
                String::new()
            };

            // Detect if this is a video or audio format
            let is_video = !resolution.contains("audio only")
                || line.to_lowercase().contains("video only")
                || (line.contains("mp4") && !line.to_lowercase().contains("audio only"));

            let is_audio = resolution.contains("audio only")
                || line.to_lowercase().contains("audio only")
                || extension == "m4a"
                || extension == "mp3"
                || extension == "ogg"
                || extension == "opus";

            // Only add if it's a real format (not a header or separator),
            // and skip mhtml storyboard "formats" which aren't real video/audio.
            if !id.contains('-') && !id.contains('=') && extension != "mhtml" {
                formats.push(Format {
                    id,
                    extension,
                    resolution,
                    description,
                    is_video,
                    is_audio,
                });
            }
        }
    }

    formats
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
