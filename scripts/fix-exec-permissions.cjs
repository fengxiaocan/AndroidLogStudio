#!/usr/bin/env node
// afterPack hook for electron-builder
// Ensures the Rust engine and ADB binaries are executable after packaging.

const { chmodSync, existsSync } = require('node:fs');
const { join } = require('node:path');

module.exports = async function (context) {
  const { appOutDir, packager, electronPlatformName } = context;

  // Only relevant on non-Windows
  if (electronPlatformName === 'win32') {
    return;
  }

  const resourcesPath = packager.platform === 'darwin'
    ? join(appOutDir, `${packager.appInfo.productFilename}.app`, 'Contents', 'Resources')
    : join(appOutDir, 'resources');   // linux + others

  const candidates = [
    join(resourcesPath, 'engine', 'als-engine'),
    join(resourcesPath, 'engine', 'als-engine.exe'),
    join(resourcesPath, 'libs', 'linux', 'adb'),
    join(resourcesPath, 'libs', 'macos', 'adb'),
    join(resourcesPath, 'libs', 'windows', 'adb.exe'),
  ];

  for (const file of candidates) {
    if (existsSync(file)) {
      try {
        chmodSync(file, 0o755);
        console.log(`[afterPack] Ensured executable: ${file}`);
      } catch (err) {
        console.warn(`[afterPack] Failed to chmod ${file}: ${err.message}`);
      }
    }
  }
};
