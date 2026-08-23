const { invoke } = window.__TAURI__.core;

const state = {
  videoFormats: [],
  audioFormats: [],
  // Mirrors the backend's AppConfig. save_config replaces the whole file, so
  // we keep the full object here and merge into it on every change instead
  // of sending single-field patches that would wipe out the other fields.
  config: { output_dir: "", preferred_audio_language: null },
};

window.addEventListener("DOMContentLoaded", () => {
  checkYtDlp();
  loadConfig();

  document.getElementById("paste-btn").addEventListener("click", pasteUrl);
  document.getElementById("browse-btn").addEventListener("click", browseDir);
  document.getElementById("fetch-btn").addEventListener("click", fetchFormats);
  document.getElementById("download-btn").addEventListener("click", startDownload);
  document
    .querySelectorAll('input[name="download-type"]')
    .forEach((el) => el.addEventListener("change", updateDownloadTypeUI));
  document.getElementById("output-dir").addEventListener("change", (e) => {
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
    if (config.output_dir) {
      document.getElementById("output-dir").value = config.output_dir;
    }
    document.getElementById("preferred-audio-language").value =
      config.preferred_audio_language || "";
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

function setStatus(message, cls) {
  const el = document.getElementById("status");
  el.textContent = message;
  el.className = cls ? `var-${cls}` : "";
  el.style.color = cls
    ? getComputedStyle(document.documentElement).getPropertyValue(`--${cls}`)
    : "";
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

function updateDownloadTypeUI() {
  const type = document.querySelector('input[name="download-type"]:checked').value;
  document.getElementById("fetch-btn").parentElement.style.display =
    type === "video_audio" ? "flex" : "none";
  if (type !== "video_audio") {
    document.getElementById("video-format-row").style.display = "none";
    document.getElementById("audio-format-row").style.display = "none";
  }
}

async function fetchFormats() {
  const url = document.getElementById("url").value.trim();
  if (!url) {
    setStatus("Please enter a URL first", "err");
    return;
  }

  setStatus("Fetching available formats...", "warn");
  setProcessing(true);

  try {
    const result = await invoke("fetch_formats", { url });
    state.videoFormats = result.video;
    state.audioFormats = result.audio;

    populateFormatSelect("video-format", "video-format-row", state.videoFormats);
    populateFormatSelect("audio-format", "audio-format-row", state.audioFormats);
    selectBestVideoFormat();
    selectBestAudioFormat();
    document.getElementById("format-lists").style.display = "block";
    renderFormatTables();

    setStatus("Formats fetched successfully", "ok");
  } catch (e) {
    setStatus(e, "err");
  } finally {
    setProcessing(false);
  }
}

function populateFormatSelect(selectId, rowId, formats) {
  const sel = document.getElementById(selectId);
  sel.innerHTML = "";
  for (const f of formats) {
    const opt = document.createElement("option");
    opt.value = f.id;
    opt.textContent = `${f.id} - ${f.description} (${f.extension})`;
    sel.appendChild(opt);
  }
  document.getElementById(rowId).style.display = formats.length ? "flex" : "none";
}

function selectBestVideoFormat() {
  let best = null;
  let bestArea = -1;

  for (const f of state.videoFormats) {
    const match = /^(\d+)x(\d+)$/.exec((f.resolution || "").trim());
    if (!match) continue;
    const area = parseInt(match[1], 10) * parseInt(match[2], 10);
    if (area > bestArea) {
      bestArea = area;
      best = f;
    }
  }

  if (best) {
    document.getElementById("video-format").value = best.id;
  }
}

// Picks an audio track to preselect when a video offers more than one.
// Priority: 1) a track matching the user's preferred language (if set),
// 2) among those, the one the creator marked as default/original,
// 3) otherwise just the first track yt-dlp reported.
function selectBestAudioFormat() {
  if (!state.audioFormats.length) return;

  const preferredLang = (state.config.preferred_audio_language || "").toLowerCase();
  let candidates = state.audioFormats;

  if (preferredLang) {
    // Base-language match (e.g. preference "en" matches track language "en-US").
    const matches = state.audioFormats.filter((f) =>
      (f.language || "").toLowerCase().startsWith(preferredLang),
    );
    if (matches.length) {
      candidates = matches;
    }
    // If nothing matches the preferred language, fall through to all
    // candidates rather than leaving the dropdown on its browser default
    // (usually the lowest-quality track) — matches "prefer creator's default
    // when the preference can't be satisfied."
  }

  const chosen = candidates.find((f) => f.is_default_audio) || candidates[0];
  document.getElementById("audio-format").value = chosen.id;
}

function renderFormatTables() {
  renderTable("video-format-table", state.videoFormats, ["id", "extension", "resolution", "description"]);
  renderTable("audio-format-table", state.audioFormats, ["id", "extension", "description"]);
}

function renderTable(containerId, formats, cols) {
  const container = document.getElementById(containerId);
  if (!formats.length) {
    container.innerHTML = "<p>None found.</p>";
    return;
  }
  let html = "<table><thead><tr>";
  html += cols.map((c) => `<th>${c}</th>`).join("");
  html += "</tr></thead><tbody>";
  for (const f of formats) {
    html += "<tr>" + cols.map((c) => `<td>${f[c]}</td>`).join("") + "</tr>";
  }
  html += "</tbody></table>";
  container.innerHTML = html;
}

async function startDownload() {
  const url = document.getElementById("url").value.trim();
  const outputDir = document.getElementById("output-dir").value.trim();
  const downloadType = document.querySelector('input[name="download-type"]:checked').value;

  setStatus("Downloading...", "warn");
  setProcessing(true);

  try {
    await invoke("download", {
      url,
      outputDir,
      downloadType,
      videoFormat: downloadType === "video_audio" ? document.getElementById("video-format").value : null,
      audioFormat: downloadType === "video_audio" ? document.getElementById("audio-format").value : null,
    });
    setStatus("Download completed successfully", "ok");
  } catch (e) {
    setStatus(e, "err");
  } finally {
    setProcessing(false);
  }
}
