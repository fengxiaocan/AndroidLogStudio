import { app, BrowserWindow, ipcMain } from 'electron';
import path from 'node:path';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const mainDir = path.dirname(fileURLToPath(import.meta.url));
const engineToken = randomUUID();

let engineProcess: ChildProcessWithoutNullStreams | null = null;
let engineUrl = '';
let resolveEngineReady: ((url: string) => void) | null = null;
let rejectEngineReady: ((error: Error) => void) | null = null;
const engineReady = new Promise<string>((resolve, reject) => {
  resolveEngineReady = resolve;
  rejectEngineReady = reject;
});

function engineBinaryPath() {
  const executable = process.platform === 'win32' ? 'als-engine.exe' : 'als-engine';

  if (app.isPackaged) {
    return path.join(process.resourcesPath, 'engine', executable);
  }

  return path.join(process.cwd(), 'target', 'debug', executable);
}

function failEngineStartup(message: string) {
  const error = new Error(message);
  console.error(`[als-engine] ${message}`);
  rejectEngineReady?.(error);
  rejectEngineReady = null;
  resolveEngineReady = null;
}

function startEngine() {
  if (engineProcess) {
    return;
  }

  engineProcess = spawn(engineBinaryPath(), [], {
    env: { ...process.env, ALS_ENGINE_TOKEN: engineToken },
  });

  engineProcess.stdout.on('data', (data: Buffer) => {
    const output = data.toString();
    const readyMatch = output.match(/ALS_ENGINE_READY port=(\d+)/);

    if (readyMatch) {
      engineUrl = `ws://127.0.0.1:${readyMatch[1]}/ws?token=${encodeURIComponent(engineToken)}`;
      resolveEngineReady?.(engineUrl);
      resolveEngineReady = null;
      rejectEngineReady = null;
    }
  });

  engineProcess.stderr.on('data', (data: Buffer) => {
    console.error(`[als-engine] ${data.toString().trim()}`);
  });

  engineProcess.on('error', (error) => {
    engineProcess = null;
    engineUrl = '';
    failEngineStartup(`failed to start ${engineBinaryPath()}: ${error.message}`);
  });

  engineProcess.on('exit', (code, signal) => {
    engineProcess = null;
    engineUrl = '';

    if (resolveEngineReady) {
      failEngineStartup(`engine exited before readiness (code=${code ?? 'none'}, signal=${signal ?? 'none'})`);
    }
  });
}

async function createWindow() {
  const win = new BrowserWindow({
    width: 1400,
    height: 900,
    webPreferences: {
      // Preload must be CJS (.cjs). With package.json "type":"module", a
      // .js preload is treated as ESM and fails silently under sandbox,
      // leaving window.als undefined in the renderer.
      preload: path.join(mainDir, 'preload.cjs'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
  });

  if (process.env.VITE_DEV_SERVER_URL) {
    await win.loadURL(process.env.VITE_DEV_SERVER_URL);
  } else {
    await win.loadFile(path.join(mainDir, '../renderer/index.html'));
  }
}

ipcMain.handle('engine:get-url', async () => engineUrl || engineReady);

app.whenReady().then(() => {
  startEngine();
  return createWindow();
});

app.on('before-quit', () => {
  engineProcess?.kill();
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});
