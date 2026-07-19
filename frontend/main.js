const { invoke } = window.__TAURI__.core;

const state = {
  videoFormats: [],
  audioFormats: [],
};

window.addEventListener("DOMContentLoaded", () => {
  checkYtDlp();
  loadSavedOutputDir();

  document.getElementById("paste-btn").addEventListener("click", pasteUrl);
  document.getElementById("browse-btn").addEventListener("click", browseDir);
  document.getElementById("fetch-btn").addEventListener("click", fetchFormats);
  document.getElementById("download-btn").addEventListener("click", startDownload);
  document
    .querySelectorAll('input[name="download-type"]')
    .forEach((el) => el.addEventListener("change", updateDownloadTypeUI));
  document.getElementById("output-dir").addEventListener("change", (e) => {
    saveOutputDir(e.target.value.trim());
  });

  updateDownloadTypeUI();
});

async function loadSavedOutputDir() {
  try {
    const config = await invoke("load_config");
    if (config.output_dir) {
      document.getElementById("output-dir").value = config.output_dir;
    }
  } catch (e) {
    console.error("Failed to load saved config", e);
  }
}

async function saveOutputDir(outputDir) {
  try {
    await invoke("save_config", { config: { output_dir: outputDir } });
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
      saveOutputDir(dir);
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
