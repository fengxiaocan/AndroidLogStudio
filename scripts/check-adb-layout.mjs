#!/usr/bin/env node
import { accessSync, constants, existsSync, statSync } from 'node:fs';
import { join } from 'node:path';

const allowMissing = process.argv.includes('--allow-missing');
const projectRoot = process.cwd();

const expected = [
  { platform: 'linux', path: 'libs/linux/adb', executable: true },
  { platform: 'macos', path: 'libs/macos/adb', executable: true },
  { platform: 'windows', path: 'libs/windows/adb.exe', executable: false },
  { platform: 'windows', path: 'libs/windows/AdbWinApi.dll', executable: false },
  { platform: 'windows', path: 'libs/windows/AdbWinUsbApi.dll', executable: false },
];

const legacyDirectories = [
  { from: 'libs/mac', to: 'libs/macos' },
  { from: 'libs/win', to: 'libs/windows' },
];

const missing = [];
const notExecutable = [];
const legacy = [];

for (const entry of expected) {
  const absolutePath = join(projectRoot, entry.path);
  if (!existsSync(absolutePath)) {
    missing.push(entry);
    continue;
  }

  if (entry.executable) {
    try {
      accessSync(absolutePath, constants.X_OK);
    } catch {
      notExecutable.push(entry);
    }
  }
}

for (const entry of legacyDirectories) {
  const absolutePath = join(projectRoot, entry.from);
  if (existsSync(absolutePath) && statSync(absolutePath).isDirectory()) {
    legacy.push(entry);
  }
}

if (legacy.length > 0) {
  console.warn('Legacy ADB library directories detected:');
  for (const entry of legacy) {
    console.warn(`- ${entry.from} should be copied or renamed to ${entry.to}`);
  }
}

if (missing.length === 0 && notExecutable.length === 0) {
  console.log('ADB library layout is complete:');
  for (const entry of expected) {
    console.log(`- ${entry.platform}: ${entry.path}`);
  }
  process.exit(0);
}

if (missing.length > 0) {
  console.warn('Missing canonical ADB files:');
  for (const entry of missing) {
    console.warn(`- ${entry.platform}: ${entry.path}`);
  }
}

if (notExecutable.length > 0) {
  console.warn('ADB binaries are present but not executable:');
  for (const entry of notExecutable) {
    console.warn(`- ${entry.platform}: ${entry.path}`);
  }
}

if (allowMissing && notExecutable.length === 0) {
  console.warn('Continuing because --allow-missing was provided.');
  process.exit(0);
}

console.error('ADB library layout check failed. See docs/ADB-LIBS.md for setup details.');
process.exit(1);
