// One-shot Electron smoke: launch the app against the real X server,
// wait for the renderer + engine WebSocket, screenshot it, dump key UI
// text and diagnostics, then exit.
// Local dev-only verification harness — no untrusted input.
import { _electron as electron } from 'playwright-core';
import * as fs from 'node:fs';
import * as path from 'node:path';

const APP_DIR = path.resolve(import.meta.dirname, '..');
const SHOT_DIR = process.env.SCREENSHOT_DIR || '/tmp/shots';
fs.mkdirSync(SHOT_DIR, { recursive: true });
const electronBin = path.join(APP_DIR, 'node_modules/electron/dist/electron');

function log(...a) { console.log(...a); }

const app = await electron.launch({
  executablePath: electronBin,
  args: ['--no-sandbox', APP_DIR],
  cwd: APP_DIR,
  env: { ...process.env, DISPLAY: process.env.DISPLAY || ':0' },
  timeout: 30_000,
});
log('launched');

// Capture MAIN process stdout/stderr — engine spawn errors land here.
const proc = app.process();
proc.stdout?.on('data', (d) => log('[main:out]', d.toString().trim()));
proc.stderr?.on('data', (d) => log('[main:err]', d.toString().trim()));

const page = await app.firstWindow();

// Capture renderer diagnostics.
page.on('console', (m) => log(`[console:${m.type()}]`, m.text()));
page.on('pageerror', (e) => log('[pageerror]', e.message));

await page.waitForLoadState('domcontentloaded').catch(() => {});

try {
  await page.waitForSelector('text=Android Logcat Studio', { timeout: 15_000 });
  log('shell heading visible');
} catch {
  log('WARN: shell heading not found within 15s');
}

// Resolve the engine URL via IPC (main → renderer bridge).
const engineUrl = await page.evaluate(async () => {
  try { return await window.als?.getEngineUrl?.(); }
  catch (e) { return 'ERR: ' + e.message; }
});
log('engineUrl:', engineUrl);

// Poll the status bar until it leaves the DISCONNECTED/pending state, up to ~20s.
let statusBar = null;
for (let i = 0; i < 40; i++) {
  statusBar = await page.evaluate(
    () => document.querySelector('.status-bar')?.innerText ?? null,
  );
  if (statusBar && !/DISCONNECTED/i.test(statusBar)) break;
  await new Promise((r) => setTimeout(r, 500));
}

const shot = path.join(SHOT_DIR, 'als-launch.png');
await page.screenshot({ path: shot });
log('screenshot:', shot);

const probe = await page.evaluate(() => {
  return {
    heading: document.querySelector('h1')?.textContent?.trim() ?? null,
    statusBar: document.querySelector('.status-bar')?.innerText ?? null,
    deviceTabs: [...document.querySelectorAll('.device-tab, [class*="device-tab"]')]
      .map((e) => e.innerText.replace(/\s+/g, ' ').trim()).slice(0, 8),
    logRows: document.querySelectorAll('[class*="log-row"], [class*="log-entry"]').length,
    bodyLen: document.body.innerText.length,
  };
});
log('probe:', JSON.stringify(probe, null, 2));

await app.close();
log('closed');
