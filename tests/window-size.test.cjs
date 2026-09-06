const { test } = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');

function setup({ scale = 1, y = 40, screenHeight = 1200, maximized = false, fullscreen = false } = {}) {
  const sizes = [];
  const current = {
    isMaximized: async () => maximized, isFullscreen: async () => fullscreen,
    innerSize: async () => ({ width: 900 * scale, height: 700 * scale }),
    outerSize: async () => ({ width: 900 * scale, height: 730 * scale }),
    outerPosition: async () => ({ x: 0, y: y * scale }),
    setSize: async size => sizes.push(size),
  };
  const context = vm.createContext({ window: {
    innerHeight: 700, addEventListener() {},
    __TAURI__: { window: { getCurrentWindow: () => current, currentMonitor: async () => ({
      scaleFactor: scale, workArea: { position: { y: 0 }, size: { height: screenHeight * scale } },
    }) }, dpi: { PhysicalSize: class { constructor(width, height) { this.width = width; this.height = height; } } } },
  } });
  vm.runInContext(fs.readFileSync('frontend/window-size.js', 'utf8'), context);
  return { sizes, grow: height => vm.runInContext(`growWindowToContent(${height})`, context) };
}

test('grows for additional controls at normal and Retina scale, preserving width', async () => {
  for (const scale of [1, 2]) {
    const app = setup({ scale });
    await app.grow(850);
    assert.equal(app.sizes[0].height, 850 * scale);
    assert.equal(app.sizes[0].width, 900 * scale);
  }
});
test('growth stops above the dock and accounts for title bar and current position', async () => {
  const app = setup({ y: 100, screenHeight: 1000 });
  await app.grow(3000);
  assert.equal(app.sizes[0].height, 862);
});
test('does not shrink or change maximized and fullscreen windows', async () => {
  for (const options of [{}, { maximized: true }, { fullscreen: true }, { y: 500, screenHeight: 1000 }]) {
    const app = setup(options);
    await app.grow(options.maximized || options.fullscreen || options.y ? 900 : 500);
    assert.equal(app.sizes.length, 0);
  }
});
