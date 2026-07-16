// One-shot settings panel smoke: open settings, toggle column, switch locale, screenshot.
import { _electron as electron } from 'playwright-core';
import * as fs from 'node:fs';
import * as path from 'node:path';

const APP_DIR = path.resolve(import.meta.dirname, '..');
const SHOT_DIR = process.env.SCREENSHOT_DIR || '/tmp/shots';
fs.mkdirSync(SHOT_DIR, { recursive: true });
const electronBin = path.join(APP_DIR, 'node_modules/electron/dist/electron');

function log(...a) {
  console.log(...a);
}

const app = await electron.launch({
  executablePath: electronBin,
  args: ['--no-sandbox', APP_DIR],
  cwd: APP_DIR,
  env: { ...process.env, DISPLAY: process.env.DISPLAY || ':0' },
  timeout: 30_000,
});
log('launched');

const proc = app.process();
proc.stdout?.on('data', (d) => log('[main:out]', d.toString().trim()));
proc.stderr?.on('data', (d) => log('[main:err]', d.toString().trim()));

const page = await app.firstWindow();
page.on('console', (m) => log(`[console:${m.type()}]`, m.text()));
page.on('pageerror', (e) => log('[pageerror]', e.message));

await page.waitForLoadState('domcontentloaded').catch(() => {});
await page.waitForSelector('text=Android Logcat Studio', { timeout: 15_000 });
log('shell visible');

// Clear any prior settings so the run is deterministic.
await page.evaluate(() => localStorage.removeItem('als.settings.v1'));
await page.reload();
await page.waitForSelector('text=Android Logcat Studio', { timeout: 15_000 });

// Default locale is zh — settings button should say 设置.
const settingsBtn = page.getByRole('button', { name: '设置' });
await settingsBtn.click();
await page.waitForSelector('.settings-panel', { timeout: 5_000 });
log('settings panel open');

await page.screenshot({ path: path.join(SHOT_DIR, 'als-settings-zh.png') });

// Hide PID column.
const pidCheckbox = page.locator('.settings-check').filter({ hasText: 'PID' }).locator('input');
await pidCheckbox.uncheck();
log('pid column unchecked');

// Switch language to English.
await page.locator('select').filter({ has: page.locator('option[value="en"]') }).selectOption('en');
await page.waitForSelector('text=Settings', { timeout: 5_000 });
log('locale switched to en');

// Title + chrome should be English now.
const title = await page.locator('.settings-panel h2').textContent();
const closeLabel = await page.locator('.settings-panel__header button').textContent();
log('panel title:', title?.trim(), 'close:', closeLabel?.trim());

await page.screenshot({ path: path.join(SHOT_DIR, 'als-settings-en.png') });

// Close settings.
await page.getByRole('button', { name: 'Close' }).click();
await page.waitForSelector('.settings-panel', { state: 'detached', timeout: 5_000 });
log('settings closed');

// Toolbar should now show English labels.
const toolbarText = await page.evaluate(() => document.querySelector('.toolbar')?.innerText ?? '');
log('toolbar:', toolbarText.replace(/\s+/g, ' ').trim());

// Persist check: reload and confirm locale/en + pid hidden.
await page.reload();
await page.waitForSelector('text=Android Logcat Studio', { timeout: 15_000 });
const stored = await page.evaluate(() => localStorage.getItem('als.settings.v1'));
log('stored settings:', stored);

await page.getByRole('button', { name: 'Settings' }).click();
await page.waitForSelector('.settings-panel', { timeout: 5_000 });
const pidChecked = await page
  .locator('.settings-check')
  .filter({ hasText: 'PID' })
  .locator('input')
  .isChecked();
const localeValue = await page.locator('select').filter({ has: page.locator('option[value="en"]') }).inputValue();
log('after reload: pidChecked=', pidChecked, 'locale=', localeValue);

await page.screenshot({ path: path.join(SHOT_DIR, 'als-settings-persisted.png') });

const ok = !pidChecked && localeValue === 'en';
await app.close();
log(ok ? 'SMOKE OK' : 'SMOKE FAIL');
process.exit(ok ? 0 : 1);
