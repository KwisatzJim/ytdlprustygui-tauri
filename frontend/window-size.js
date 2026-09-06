// Grow downward as content appears, preserving the user's width and position.
// Keep scrolling when the content cannot fit on the current screen.
async function growWindowToContent(contentHeight) {
  const api = window.__TAURI__;
  const current = api.window.getCurrentWindow();
  const [maximized, fullscreen, monitor, inner, outer, position] = await Promise.all([
    current.isMaximized(), current.isFullscreen(), api.window.currentMonitor(),
    current.innerSize(), current.outerSize(), current.outerPosition(),
  ]);
  if (maximized || fullscreen || !monitor || !window.innerHeight) return;

  // DOM measurements use CSS pixels; native sizes and monitor bounds are physical.
  const ratio = inner.height / window.innerHeight;
  const desired = Math.ceil(contentHeight * ratio);
  const frameHeight = Math.max(0, outer.height - inner.height);
  const area = monitor.workArea;
  const available = Math.floor(area.position.y + area.size.height - position.y
    - frameHeight - 8 * monitor.scaleFactor);
  const height = Math.min(desired, available);
  if (height > inner.height + 2) {
    await current.setSize(new api.dpi.PhysicalSize(inner.width, height));
  }
}

window.addEventListener("DOMContentLoaded", () => {
  const main = document.querySelector("main");
  let previousHeight = 0;
  let pendingHeight = null;
  let running = false;
  async function resize() {
    if (running) return;
    running = true;
    try {
      while (pendingHeight !== null) {
        const height = pendingHeight;
        pendingHeight = null;
        await growWindowToContent(height);
      }
    } catch (error) {
      console.warn("Could not resize window to fit content", error);
    } finally {
      running = false;
    }
  }
  new ResizeObserver(() => {
    const height = Math.ceil(main.getBoundingClientRect().height);
    if (height === previousHeight) return;
    previousHeight = height;
    pendingHeight = height;
    void resize();
  }).observe(main);
});
