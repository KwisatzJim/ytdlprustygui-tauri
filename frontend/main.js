const { invoke, Channel } = window.__TAURI__.core;

const state = {
  videoFormats: [],
  audioFormats: [],
  formatsUrl: null,
  formatsRevision: 0,
  downloadSequence: 0,
  activeDownload: null,
  completedOutputDir: null,
  // Mirrors the backend's AppConfig. save_config replaces the whole file, so
  // we keep the full object here and merge into it on every change instead
  // of sending single-field patches that would wipe out the other fields.
  config: { output_dir: "", preferred_audio_language: null, preferred_video_resolution: null, preferred_audio_quality: "high", theme: "system" },
};

window.addEventListener("DOMContentLoaded", () => {
  checkDependencies();
  loadConfig();
  document.getElementById("theme").addEventListener("change", (e) => {
    applyTheme(e.target.value);
    saveConfig({ theme: e.target.value });
  });

  document.getElementById("paste-btn").addEventListener("click", pasteUrl);
  document.getElementById("url").addEventListener("input", invalidateFormats);
  document.getElementById("browse-btn").addEventListener("click", browseDir);
  document.getElementById("fetch-btn").addEventListener("click", fetchFormats);
  document.getElementById("download-btn").addEventListener("click", startDownload);
  document.getElementById("cancel-btn").addEventListener("click", cancelDownload);
  document.getElementById("open-folder-btn").addEventListener("click", openOutputFolder);
  document.getElementById("preferred-audio-quality").addEventListener("change", (e) => {
    saveConfig({ preferred_audio_quality: e.target.value });
    selectBestAudioFormat();
  });
  document.getElementById("preferred-video-resolution").addEventListener("change", (e) => {
    saveConfig({ preferred_video_resolution: Number(e.target.value) || null });
    selectBestVideoFormat();
  });
  document
    .querySelectorAll('input[name="download-type"]')
    .forEach((el) => el.addEventListener("change", updateDownloadTypeUI));
  document.getElementById("output-dir").addEventListener("change", (e) => {
    state.completedOutputDir = null;
    document.getElementById("open-folder-btn").style.display = "none";
    saveConfig({ output_dir: e.target.value.trim() });
  });
  document.getElementById("preferred-audio-language").addEventListener("change", (e) => {
    saveConfig({ preferred_audio_language: e.target.value || null });
    // Re-pick immediately if formats are already loaded, so changing the
    // preference doesn't require a re-fetch to take effect.
    if (state.audioFormats.length) {
      selectBestAudioFormat();
    }
  });

  updateDownloadTypeUI();
});

async function loadConfig() {
  try {
    const config = await invoke("load_config");
    state.config = config;
    document.getElementById("theme").value = config.theme || "system";
    applyTheme(config.theme || "system");
    if (config.output_dir) {
      document.getElementById("output-dir").value = config.output_dir;
    }
    document.getElementById("preferred-audio-language").value =
      config.preferred_audio_language || "";
    document.getElementById("preferred-video-resolution").value =
      config.preferred_video_resolution || "";
    selectBestVideoFormat();
    document.getElementById("preferred-audio-quality").value =
      config.preferred_audio_quality || "high";
    selectBestAudioFormat();
  } catch (e) {
    console.error("Failed to load saved config", e);
  }
}

async function saveConfig(patch) {
  state.config = { ...state.config, ...patch };
  try {
    await invoke("save_config", { config: state.config });
  } catch (e) {
    console.error("Failed to save config", e);
  }
}

async function checkYtDlp() {
  try {
    const version = await invoke("check_ytdlp");
    setStatus(`Ready (yt-dlp ${version})`, "");
  } catch (e) {
    setStatus(e, "err");
  }
}

async function checkDependencies() {
  try {
    const dependencies = await invoke("check_dependencies");
    for (const dependency of dependencies) {
      const id = dependency.name.toLowerCase().replace("-", "");
      const element = document.getElementById(`dependency-${id}`);
      element.textContent = `${dependency.name}: ${dependency.available ? "Ready" : "Missing"}`;
      element.className = `dependency ${dependency.available ? "ok" : "err"}`;
      element.title = dependency.detail;
    }
    const missing = dependencies.filter((dependency) => !dependency.available);
    if (missing.length) {
      setStatus(`Missing required tools: ${missing.map((dependency) => dependency.name).join(", ")}`, "err");
    } else {
      setStatus("Ready to download", "ok");
    }
  } catch (error) {
    setStatus(`Could not check required tools: ${error}`, "err");
  }
}

function applyTheme(theme) {
  document.documentElement.dataset.theme = theme;
}

function setStatus(message, cls) {
  const el = document.getElementById("status");
  el.textContent = message;
  el.className = cls ? `var-${cls}` : "";
}

function setProcessing(isProcessing) {
  document.getElementById("spinner").style.display = isProcessing ? "inline-block" : "none";
  document.getElementById("fetch-btn").disabled = isProcessing;
  document.getElementById("download-btn").disabled = isProcessing;
}

async function pasteUrl() {
  try {
    const text = await invoke("plugin:clipboard-manager|read_text");
    document.getElementById("url").value = text;
    invalidateFormats();
  } catch (e) {
    setStatus("Failed to paste from clipboard", "err");
  }
}

async function browseDir() {
  try {
    const dir = await invoke("plugin:dialog|open", {
      options: {
        directory: true,
        multiple: false,
      },
    });
    if (dir) {
      document.getElementById("output-dir").value = dir;
      saveConfig({ output_dir: dir });
    }
  } catch (e) {
    console.error(e);
    setStatus(`Failed to open folder dialog: ${e}`, "err");
  }
}

function invalidateFormats() {
  state.formatsRevision += 1;
  state.formatsUrl = null;
  state.videoFormats = [];
  state.audioFormats = [];
  for (const id of ["video-format", "audio-format", "video-format-table", "audio-format-table"]) {
    document.getElementById(id).innerHTML = "";
  }
  updateDownloadTypeUI();
}

function updateDownloadTypeUI() {
  const type = document.querySelector('input[name="download-type"]:checked').value;
  const showFormats = type === "video_audio";
  document.getElementById("playlist-note").hidden = showFormats;
  document.getElementById("fetch-btn").parentElement.style.display =
    showFormats ? "flex" : "none";
  document.getElementById("video-format-row").style.display =
    showFormats && state.videoFormats.length ? "flex" : "none";
  document.getElementById("audio-format-row").style.display =
    showFormats && state.audioFormats.length ? "flex" : "none";
  document.getElementById("format-lists").style.display =
    showFormats && (state.videoFormats.length || state.audioFormats.length) ? "block" : "none";
}

async function fetchFormats() {
  const url = document.getElementById("url").value.trim();
  if (!url) {
    setStatus("Please enter a URL first", "err");
    return;
  }

  setStatus("Fetching available formats...", "warn");
  invalidateFormats();
  const revision = state.formatsRevision;
  const isCurrent = () => revision === state.formatsRevision
    && url === document.getElementById("url").value.trim();
  setProcessing(true);

  try {
    const result = await invoke("fetch_formats", { url });
    if (!isCurrent()) return;
    state.formatsUrl = url;
    state.videoFormats = result.video;
    state.audioFormats = result.audio;

    populateFormatSelect("video-format", "video-format-row", state.videoFormats);
    populateFormatSelect("audio-format", "audio-format-row", state.audioFormats);
    selectBestVideoFormat();
    selectBestAudioFormat();
    renderFormatTables();
    updateDownloadTypeUI();

    setStatus("Formats fetched successfully", "ok");
  } catch (e) {
    if (isCurrent()) setStatus(e, "err");
  } finally {
    if (!isCurrent()) setStatus("URL changed. Fetch formats for the current video.", "");
    setProcessing(false);
  }
}

function populateFormatSelect(selectId, rowId, formats) {
  const sel = document.getElementById(selectId);
  sel.innerHTML = "";
  for (const f of formats) {
    const opt = document.createElement("option");
    opt.value = f.id;
    const resolution = selectId === "video-format" ? `${f.resolution} · ` : "";
    opt.textContent = `${f.id} - ${resolution}${f.description} (${f.extension})`;
    sel.appendChild(opt);
  }
  document.getElementById(rowId).style.display = formats.length ? "flex" : "none";
}

function selectBestVideoFormat() {
  // Use the shorter dimension so portrait video follows the same preference.
  const ranked = state.videoFormats.flatMap((format) => {
    const match = /^(\d+)x(\d+)$/.exec((format.resolution || "").trim());
    if (!match) return [];
    const width = Number(match[1]);
    const height = Number(match[2]);
    if (!width || !height) return [];
    return [{ format, resolution: Math.min(width, height), area: width * height }];
  }).sort((a, b) => b.resolution - a.resolution || b.area - a.area);
  const preferred = state.config.preferred_video_resolution;
  const best = preferred
    ? ranked.find((entry) => entry.resolution <= preferred) || ranked.at(-1)
    : ranked[0];
  const chosen = best?.format || state.videoFormats[0];
  if (chosen) document.getElementById("video-format").value = chosen.id;
}

// Picks an audio track to preselect when a video offers more than one.
// Priority: 1) a track matching the user's preferred language (if set),
// 2) among those, the one the creator marked as default/original,
// 3) quality within that group, using yt-dlp's worst-to-best ordering.
function selectBestAudioFormat() {
  if (!state.audioFormats.length) return;

  const preferredLang = (state.config.preferred_audio_language || "").toLowerCase();
  let candidates = state.audioFormats;

  if (preferredLang) {
    // Base-language match (e.g. preference "en" matches track language "en-US").
    const matches = state.audioFormats.filter((f) =>
      (f.language || "").toLowerCase() === preferredLang
        || (f.language || "").toLowerCase().startsWith(`${preferredLang}-`),
    );
    if (matches.length) {
      candidates = matches;
    }
    // If nothing matches the preferred language, fall through to all
    // candidates rather than leaving the dropdown on its browser default
    // (usually the lowest-quality track) — matches "prefer creator's default
    // when the preference can't be satisfied."
  }

  const defaults = candidates.filter((f) => f.is_default_audio);
  if (defaults.length) candidates = defaults;
  // Avoid selecting a second video stream when separate audio is available.
  const audioOnly = candidates.filter((f) => !f.is_video);
  if (audioOnly.length) candidates = audioOnly;
  const chosen = state.config.preferred_audio_quality === "low"
    ? candidates[0] : candidates.at(-1);
  document.getElementById("audio-format").value = chosen.id;
}

function renderFormatTables() {
  renderTable("video-format-table", "Available video formats", state.videoFormats, ["id", "extension", "resolution", "description"]);
  renderTable("audio-format-table", "Available audio formats", state.audioFormats, ["id", "extension", "description"]);
}

// Keep site-provided metadata as literal text inside the table markup.
function escapeHtml(value) {
  const entities = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };
  return String(value ?? "").replace(/[&<>"']/g, (character) => entities[character]);
}

function renderTable(containerId, caption, formats, cols) {
  const container = document.getElementById(containerId);
  if (!formats.length) {
    container.innerHTML = "<p>None found.</p>";
    return;
  }
  let html = `<table><caption class="visually-hidden">${escapeHtml(caption)}</caption><thead><tr>`;
  html += cols.map((c) => `<th scope="col">${escapeHtml(c)}</th>`).join("");
  html += "</tr></thead><tbody>";
  for (const f of formats) {
    html += "<tr>" + cols.map((c) => `<td>${escapeHtml(f[c])}</td>`).join("") + "</tr>";
  }
  html += "</tbody></table>";
  container.innerHTML = html;
}

async function cancelDownload() {
  const download = state.activeDownload;
  if (!download || download.cancelling) return;
  download.cancelling = true;
  document.getElementById("cancel-btn").disabled = true;
  setStatus("Cancelling download...", "warn");
  try {
    await invoke("cancel_download", { downloadId: download.id });
    // The running download reports the final result after its processes stop.
  } catch (e) {
    if (state.activeDownload !== download) return;
    download.cancelling = false;
    document.getElementById("cancel-btn").disabled = false;
    setStatus(`Could not cancel: ${e}`, "err");
  }
}

async function startDownload() {
  if (state.activeDownload) return;
  const url = document.getElementById("url").value.trim();
  const outputDir = document.getElementById("output-dir").value.trim();
  const downloadType = document.querySelector('input[name="download-type"]:checked').value;

  if (downloadType === "video_audio" && state.formatsUrl !== url) {
    setStatus("Please fetch formats for the current video first", "err");
    return;
  }

  setStatus("Checking download requirements...", "warn");
  state.completedOutputDir = null;
  document.getElementById("open-folder-btn").style.display = "none";
  setProcessing(true);
  const download = { id: String(++state.downloadSequence), cancelling: false };
  state.activeDownload = download;
  const cancelButton = document.getElementById("cancel-btn");
  cancelButton.style.display = "inline-block";
  cancelButton.disabled = true;

  let acceptingProgress = true;
  try {
    const onProgress = new Channel();
    onProgress.onmessage = (message) => {
      if (acceptingProgress && !download.cancelling) {
        cancelButton.disabled = false;
        setStatus(message, "warn");
      }
    };
    const outcome = await invoke("download", {
      downloadId: download.id,
      onProgress,
      url,
      outputDir,
      downloadType,
      audioQuality: state.config.preferred_audio_quality || "high",
      videoFormat: downloadType === "video_audio" ? document.getElementById("video-format").value : null,
      audioFormat: downloadType === "video_audio" ? document.getElementById("audio-format").value : null,
    });
    if (outcome === "cancelled") {
      setStatus("Download cancelled. Partial files were kept; you can retry the download.", "");
    } else {
      setStatus("Download completed successfully", "ok");
      state.completedOutputDir = outputDir;
      document.getElementById("open-folder-btn").style.display = "inline-block";
    }
  } catch (e) {
    setStatus(e, "err");
  } finally {
    acceptingProgress = false;
    state.activeDownload = null;
    cancelButton.style.display = "none";
    cancelButton.disabled = true;
    setProcessing(false);
  }
}

async function openOutputFolder() {
  if (!state.completedOutputDir) return;
  try {
    await invoke("open_output_folder", { path: state.completedOutputDir });
  } catch (error) {
    setStatus(error, "err");
  }
}
