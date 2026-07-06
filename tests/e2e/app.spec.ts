import { expect, test } from '@playwright/test';
import { WebSocketServer, type WebSocket } from 'ws';

test('renders shell, adb status, and sends refresh_devices', async ({ page }) => {
  const messages: string[] = [];
  const sockets = new Set<WebSocket>();
  const server = new WebSocketServer({ port: 0 });
  const address = server.address();
  if (!address || typeof address === 'string') {
    throw new Error('expected websocket server address');
  }

  server.on('connection', (socket) => {
    sockets.add(socket);
    socket.on('close', () => sockets.delete(socket));
    socket.on('message', (message) => messages.push(String(message)));
    socket.send(JSON.stringify({
      type: 'device_list',
      devices: [{ deviceId: 'mock-device', deviceName: 'Mock Device', connected: true, source: 'mock' }],
    }));
    socket.send(JSON.stringify({
      type: 'adb_status',
      available: false,
      mode: 'mock_fallback',
      path: null,
      message: 'ADB: no online devices, using mock device',
    }));
  });

  try {
    await page.addInitScript((port) => {
      window.als = {
        version: '0.1.0',
        getEngineUrl: async () => `ws://127.0.0.1:${port}/ws`,
      };
    }, address.port);

    await page.goto('http://127.0.0.1:5173');

    await expect(page.getByRole('heading', { name: 'Android Logcat Studio' })).toBeVisible();
    await expect(page.getByText('ADB: no online devices, using mock device')).toBeVisible();
    await expect(page.getByText('Source: MOCK')).toBeVisible();

    await page.getByRole('button', { name: 'Refresh Devices' }).click();
    await expect.poll(() => messages.some((message) => message.includes('refresh_devices'))).toBe(true);
  } finally {
    for (const socket of sockets) {
      socket.close();
    }

    await new Promise<void>((resolve, reject) => {
      server.close((error) => {
        if (error) {
          reject(error);
          return;
        }
        resolve();
      });
    });
  }
});
