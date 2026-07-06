import { expect, test } from '@playwright/test';
import { WebSocket, WebSocketServer } from 'ws';

const CLOSE_TIMEOUT_MS = 1_000;

async function waitForServer(server: WebSocketServer): Promise<void> {
  if (server.address()) {
    return;
  }

  await new Promise<void>((resolve, reject) => {
    const cleanup = () => {
      server.off('listening', onListening);
      server.off('error', onError);
    };
    const onListening = () => {
      cleanup();
      resolve();
    };
    const onError = (error: Error) => {
      cleanup();
      reject(error);
    };

    server.once('listening', onListening);
    server.once('error', onError);
  });
}

async function closeSocket(socket: WebSocket): Promise<void> {
  if (socket.readyState === WebSocket.CLOSED) {
    return;
  }

  await new Promise<void>((resolve) => {
    const cleanup = () => {
      clearTimeout(timeout);
      socket.off('close', onClose);
    };
    const finish = () => {
      cleanup();
      resolve();
    };
    const onClose = () => finish();
    const timeout = setTimeout(() => {
      socket.terminate();
      finish();
    }, CLOSE_TIMEOUT_MS);

    socket.once('close', onClose);
    if (socket.readyState === WebSocket.OPEN) {
      socket.close();
    } else if (socket.readyState === WebSocket.CONNECTING) {
      socket.terminate();
    }
  });
}

async function closeServer(server: WebSocketServer | undefined, sockets: Set<WebSocket>): Promise<void> {
  await Promise.allSettled([...sockets].map((socket) => closeSocket(socket)));

  if (!server) {
    return;
  }

  await new Promise<void>((resolve) => {
    let done = false;
    const finish = () => {
      if (done) {
        return;
      }
      done = true;
      clearTimeout(timeout);
      resolve();
    };
    const timeout = setTimeout(() => {
      for (const socket of server.clients) {
        socket.terminate();
      }
      finish();
    }, CLOSE_TIMEOUT_MS);

    try {
      server.close(() => finish());
    } catch {
      finish();
    }
  });
}

test('renders shell, adb status, and sends refresh_devices', async ({ page }) => {
  const messages: string[] = [];
  const sockets = new Set<WebSocket>();
  let server: WebSocketServer | undefined;

  try {
    server = new WebSocketServer({ host: '127.0.0.1', port: 0 });
    await waitForServer(server);

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
    await closeServer(server, sockets);
  }
});
