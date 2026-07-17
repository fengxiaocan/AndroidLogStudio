import { app, BrowserWindow, dialog, ipcMain } from 'electron';
import path from 'node:path';
import fs from 'node:fs/promises';
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

  const isPackaged = app.isPackaged;
  const engineCwd = isPackaged 
    ? path.join(process.resourcesPath) 
    : process.cwd();

  const binPath = engineBinaryPath();

  // Best-effort: ensure the engine binary is executable (helps after packaging on Linux)
  try {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const { chmodSync, existsSync } = require('node:fs');
    if (existsSync(binPath)) {
      chmodSync(binPath, 0o755);
    } else if (isPackaged) {
      console.error(`[als-engine] Packaged engine binary not found at ${binPath}. Bundled exec may be misconfigured.`);
    }
  } catch {
    // ignore
  }

  // Provide a user-writable log root. In packaged installs the engine cwd (resources)
  // is usually not writable by the user.
  const logRoot = isPackaged
    ? path.join(app.getPath('userData'), 'logs')
    : path.join(process.cwd(), 'logs');

  // Provide the base directory where bundled resources (engine + libs) live.
  // In packaged: resourcesPath (where extraResources are placed)
  // In dev: project cwd
  const resourcesBase = isPackaged ? process.resourcesPath : process.cwd();

  engineProcess = spawn(binPath, [], {
    cwd: engineCwd,
    env: {
      ...process.env,
      ALS_ENGINE_TOKEN: engineToken,
      ALS_LOG_ROOT: logRoot,
      ALS_RESOURCES_PATH: resourcesBase,
    },
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
  // Resolve app icon for window (works in dev + packaged)
  const iconPath = app.isPackaged
    ? path.join(process.resourcesPath, 'icon.png') // packaged apps usually embed via electron-builder
    : path.join(process.cwd(), 'build', 'icon.png');

  const win = new BrowserWindow({
    width: 1400,
    height: 900,
    icon: iconPath,
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

async function resolveAllowedExportTempPath(tempPath: string): Promise<string | null> {
  // Resolve through realpath so symlink / ".." tricks cannot escape logs/exports.
  const exportsDirLogical = path.resolve(process.cwd(), 'logs', 'exports');
  try {
    await fs.mkdir(exportsDirLogical, { recursive: true });
    const exportsDir = await fs.realpath(exportsDirLogical);
    const candidate = await fs.realpath(path.resolve(tempPath));
    const underExports =
      candidate === exportsDir || candidate.startsWith(exportsDir + path.sep);
    // Only engine-minted export temps (*.log) are accepted.
    if (!underExports || path.extname(candidate).toLowerCase() !== '.log') {
      return null;
    }
    return candidate;
  } catch {
    return null;
  }
}

ipcMain.handle(
  'export:save',
  async (
    _event,
    payload: { tempPath?: string; defaultName?: string },
  ): Promise<{ canceled: boolean; path?: string; error?: string }> => {
    const tempPath = payload?.tempPath;
    const defaultName = payload?.defaultName || 'export.log';
    if (!tempPath || typeof tempPath !== 'string') {
      return { canceled: false, error: 'missing tempPath' };
    }

    const safeTempPath = await resolveAllowedExportTempPath(tempPath);
    if (!safeTempPath) {
      return { canceled: false, error: 'temp path not allowed' };
    }

    const result = await dialog.showSaveDialog({
      defaultPath: defaultName,
      filters: [
        { name: 'Log files', extensions: ['log', 'txt'] },
        { name: 'All files', extensions: ['*'] },
      ],
    });

    if (result.canceled || !result.filePath) {
      await fs.unlink(safeTempPath).catch(() => undefined);
      return { canceled: true };
    }

    try {
      await fs.copyFile(safeTempPath, result.filePath);
      await fs.unlink(safeTempPath).catch(() => undefined);
      return { canceled: false, path: result.filePath };
    } catch (error) {
      await fs.unlink(safeTempPath).catch(() => undefined);
      const message = error instanceof Error ? error.message : String(error);
      return { canceled: false, error: message };
    }
  },
);

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
