const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');

function setup(downloadResult, cancelResult) {
  const elements = new Map();
  const calls = [];
  let resolveFetch, rejectFetch;
  const pending = new Promise((resolve, reject) => {
    resolveFetch = resolve;
    rejectFetch = reject;
  });
  const element = (id) => {
    if (!elements.has(id)) elements.set(id, {
      value: '', innerHTML: '', style: {}, parentElement: { style: {} },
      appendChild(option) { this.value ||= option.value; },
    });
    return elements.get(id);
  };
  element('url').value = 'https://example.com/a';
  const context = vm.createContext({
    window: { __TAURI__: { core: { Channel: class {}, invoke: async (command, args) => {
      calls.push({ command, args });
      if (command === 'fetch_formats') return pending;
      if (command === 'download') return downloadResult;
      if (command === 'cancel_download') return cancelResult;
      if (command === 'plugin:clipboard-manager|read_text') return 'https://example.com/b';
    } } }, addEventListener() {} },
    document: { getElementById: element, querySelector: () => ({ value: 'video_audio' }), createElement: () => ({}) },
    getComputedStyle: () => ({ getPropertyValue: () => '' }),
  });
  vm.runInContext(fs.readFileSync('frontend/main.js', 'utf8'), context);
  return { element, calls, resolveFetch, rejectFetch, run: code => vm.runInContext(code, context) };
}
const formats = { video: [{ id: '137', resolution: '1920x1080' }], audio: [{ id: '140' }] };

test('audio-only mode explains that playlist URLs download every item', () => {
  const app = setup();
  assert.equal(app.element('playlist-note').hidden, undefined);
  app.run(`document.querySelector = () => ({ value: 'audio_only' }); updateDownloadTypeUI()`);
  assert.equal(app.element('playlist-note').hidden, false);
  app.run(`document.querySelector = () => ({ value: 'video_audio' }); updateDownloadTypeUI()`);
  assert.equal(app.element('playlist-note').hidden, true);
});

test('URL editing clears loaded formats and prevents a stale download', async () => {
  const app = setup();
  const fetching = app.run('fetchFormats()');
  app.resolveFetch(formats);
  await fetching;
  assert.equal(app.element('video-format').value, '137');
  app.element('url').value = 'https://example.com/b';
  app.run('invalidateFormats()');
  assert.equal(app.element('video-format').innerHTML, '');
  assert.equal(app.element('format-lists').style.display, 'none');
  await app.run('startDownload()');
  assert.equal(app.calls.some(c => c.command === 'download'), false);
});

test('late results are discarded even when URL changes away and back', async () => {
  const app = setup();
  const fetching = app.run('fetchFormats()');
  app.element('url').value = 'https://example.com/b';
  app.run('invalidateFormats()');
  app.element('url').value = 'https://example.com/a';
  app.run('invalidateFormats()');
  app.resolveFetch(formats);
  await fetching;
  assert.equal(app.run('state.formatsUrl'), null);
  assert.equal(app.element('format-lists').style.display, 'none');
  assert.equal(app.element('fetch-btn').disabled, false);
});

test('Paste invalidates formats and late failures do not show the old URL error', async () => {
  const app = setup();
  const fetching = app.run('fetchFormats()');
  await app.run('pasteUrl()');
  app.rejectFetch('old video error');
  await fetching;
  assert.equal(app.run('state.formatsUrl'), null);
  assert.match(app.element('status').textContent, /URL changed/);
});

test('resolution preference picks the best fit and handles portrait and missing sizes', () => {
  const app = setup();
  app.run(`state.videoFormats = [
    { id: 'unknown', resolution: 'unknown' },
    { id: '4k', resolution: '3840x2160' },
    { id: 'portrait', resolution: '1080x1920' },
    { id: '720', resolution: '1280x720' }
  ]`);
  for (const [preference, expected] of [[null, '4k'], [1080, 'portrait'], [1440, 'portrait'], [480, '720'], [720, '720']]) {
    app.run(`state.config.preferred_video_resolution = ${preference}; selectBestVideoFormat()`);
    assert.equal(app.element('video-format').value, expected);
  }
  app.run(`state.videoFormats = [{ id: 'unknown', resolution: 'unknown' }]; selectBestVideoFormat()`);
  assert.equal(app.element('video-format').value, 'unknown');
});

test('saving resolution preserves output folder and audio language', async () => {
  const app = setup();
  app.run(`state.config = { output_dir: '/tmp/videos', preferred_audio_language: 'en' }`);
  await app.run('saveConfig({ preferred_video_resolution: 1080 })');
  const saved = app.calls.find(c => c.command === 'save_config').args.config;
  assert.equal(saved.output_dir, '/tmp/videos');
  assert.equal(saved.preferred_audio_language, 'en');
  assert.equal(saved.preferred_video_resolution, 1080);
});

test('audio quality follows ranking within the preferred language and default track', () => {
  const app = setup();
  app.run(`state.audioFormats = [
    { id: 'en-low', language: 'en-US', is_default_audio: true },
    { id: 'en-high', language: 'en-US', is_default_audio: true },
    { id: 'en-alternate', language: 'en-US' },
    { id: 'fr-high', language: 'fr', is_default_audio: true },
    { id: 'en-video', language: 'en-US', is_default_audio: true, is_video: true }
  ]; state.config.preferred_audio_language = 'en'; selectBestAudioFormat()`);
  assert.equal(app.element('audio-format').value, 'en-high');
  app.run(`state.config.preferred_audio_quality = 'low'; selectBestAudioFormat()`);
  assert.equal(app.element('audio-format').value, 'en-low');
  app.run(`state.audioFormats = [{ id: 'only' }]; selectBestAudioFormat()`);
  assert.equal(app.element('audio-format').value, 'only');
});

test('audio quality is saved and passed to downloads without overriding manual format', async () => {
  const app = setup();
  await app.run(`saveConfig({ preferred_audio_quality: 'low' })`);
  assert.equal(app.calls.at(-1).args.config.preferred_audio_quality, 'low');
  app.run(`state.formatsUrl = 'https://example.com/a'`);
  app.element('audio-format').value = 'manual';
  await app.run('startDownload()');
  assert.equal(app.calls.at(-1).args.audioQuality, 'low');
  assert.equal(app.calls.at(-1).args.audioFormat, 'manual');
});

test('format tables display markup as text in every metadata column', () => {
  const app = setup();
  app.run(`renderTable('video-format-table', [{
    id: '<b>137</b>', extension: 'a&b', resolution: '"quoted"',
    description: "<img src=x onerror='alert(1)'>"
  }], ['id', 'extension', 'resolution', 'description'])`);
  const html = app.element('video-format-table').innerHTML;
  assert.ok(!html.includes('<img'));
  assert.ok(!html.includes('<b>137'));
  assert.ok(html.includes('&lt;b&gt;137&lt;/b&gt;'));
  assert.ok(html.includes('a&amp;b'));
  assert.ok(html.includes('&quot;quoted&quot;'));
  assert.ok(html.includes('&#39;alert(1)&#39;'));
});

test('live progress updates while downloading and late updates cannot overwrite completion', async () => {
  let complete;
  const app = setup(new Promise(resolve => { complete = resolve; }));
  app.run(`state.formatsUrl = 'https://example.com/a'`);
  const downloading = app.run('startDownload()');
  const channel = app.calls.at(-1).args.onProgress;
  channel.onmessage('[download] 25% at 2MiB/s ETA 00:30');
  assert.match(app.element('status').textContent, /25%/);
  assert.equal(app.element('download-btn').disabled, true);
  channel.onmessage('[ExtractAudio] Destination: test.mp3');
  assert.match(app.element('status').textContent, /ExtractAudio/);
  complete();
  await downloading;
  channel.onmessage('[download] 100%');
  assert.equal(app.element('status').textContent, 'Download completed successfully');
  assert.equal(app.element('download-btn').disabled, false);
});

test('failed download keeps its error after late progress', async () => {
  let fail;
  const app = setup(new Promise((resolve, reject) => { fail = reject; }));
  app.run(`state.formatsUrl = 'https://example.com/a'`);
  const downloading = app.run('startDownload()');
  const channel = app.calls.at(-1).args.onProgress;
  fail('Download failed: conversion error');
  await downloading;
  channel.onmessage('[download] 100%');
  assert.match(app.element('status').textContent, /conversion error/);
  assert.equal(app.element('download-btn').disabled, false);
});

test('cancel waits for stopped download, ignores progress, and restores controls', async () => {
  let finish;
  const app = setup(new Promise(resolve => { finish = resolve; }));
  app.run(`state.formatsUrl = 'https://example.com/a'`);
  const running = app.run('startDownload()');
  const request = app.calls.at(-1).args;
  assert.equal(app.element('cancel-btn').disabled, true);
  request.onProgress.onmessage('Starting download...');
  assert.equal(app.element('cancel-btn').disabled, false);
  await app.run('cancelDownload()');
  assert.equal(app.calls.at(-1).args.downloadId, request.downloadId);
  request.onProgress.onmessage('[download] 75%');
  assert.equal(app.element('status').textContent, 'Cancelling download...');
  assert.equal(app.element('download-btn').disabled, true);
  await app.run('cancelDownload()');
  assert.equal(app.calls.filter(c => c.command === 'cancel_download').length, 1);
  finish('cancelled');
  await running;
  assert.match(app.element('status').textContent, /Download cancelled/);
  assert.equal(app.element('cancel-btn').style.display, 'none');
  assert.equal(app.element('download-btn').disabled, false);
  await app.run('startDownload()');
  assert.equal(app.calls.filter(c => c.command === 'download').length, 2);
});

test('completion wins when cancel arrives too late', async () => {
  let finish;
  const app = setup(new Promise(resolve => { finish = resolve; }), false);
  app.run(`state.formatsUrl = 'https://example.com/a'`);
  const running = app.run('startDownload()');
  await app.run('cancelDownload()');
  finish('completed');
  await running;
  assert.equal(app.element('status').textContent, 'Download completed successfully');
});

test('failed cancellation can be retried while the download remains active', async () => {
  let finish, rejectCancel;
  const app = setup(new Promise(resolve => { finish = resolve; }), new Promise((resolve, reject) => { rejectCancel = reject; }));
  app.run(`state.formatsUrl = 'https://example.com/a'`);
  const running = app.run('startDownload()');
  const cancelling = app.run('cancelDownload()');
  rejectCancel('command unavailable');
  await cancelling;
  assert.equal(app.element('cancel-btn').disabled, false);
  assert.match(app.element('status').textContent, /Could not cancel/);
  finish('completed');
  await running;
});
