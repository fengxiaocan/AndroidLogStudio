import { expect, test } from '@playwright/test';

test('renders the Android Logcat Studio shell', async ({ page }) => {
  await page.addInitScript(() => {
    window.als = {
      version: '0.1.0',
      getEngineUrl: async () => 'ws://127.0.0.1:9/unavailable',
    };
  });

  await page.goto('http://127.0.0.1:5173');

  await expect(page.getByRole('heading', { name: 'Android Logcat Studio' })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Log output' })).toBeVisible();
  await expect(page.getByLabel('Log statistics')).toBeVisible();
});
