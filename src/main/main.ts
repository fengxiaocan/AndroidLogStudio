import { app, BrowserWindow, ipcMain } from 'electron';
import path from 'node:path';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';

let engineProcess: ChildProcessWithoutNullStreams | null = null;
let engineUrl = '';

function engineBinaryPath() {
  const executable = process.platform === 'win32' ? 'als-engine.exe' : 'als-engine';

  if (app.isPackaged) {
    return path.join(process.resourcesPath, 'engine', executable);
  }

  return path.join(process.cwd(), 'target', 'debug', executable);
}

function startEngine() {
  if (engineProcess) {
    return;
  }

  engineProcess = spawn(engineBinaryPath());

  engineProcess.stdout.on('data', (data: Buffer) => {
    const output = data.toString();
    const readyMatch = output.match(/ALS_ENGINE_READY port=(\d+)/);

    if (readyMatch) {
      engineUrl = `ws://127.0.0.1:${readyMatch[1]}/ws`;
    }
  });

  engineProcess.stderr.on('data', (data: Buffer) => {
    console.error(`[als-engine] ${data.toString().trim()}`);
  });

  engineProcess.on('exit', () => {
    engineProcess = null;
    engineUrl = '';
  });
}

async function createWindow() {
  const win = new BrowserWindow({
    width: 1400,
    height: 900,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  if (process.env.VITE_DEV_SERVER_URL) {
    await win.loadURL(process.env.VITE_DEV_SERVER_URL);
  } else {
    await win.loadFile(path.join(__dirname, '../renderer/index.html'));
  }
}

ipcMain.handle('engine:get-url', () => engineUrl);

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
