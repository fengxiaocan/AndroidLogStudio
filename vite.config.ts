import react from '@vitejs/plugin-react';
import { defineConfig, defaultExclude } from 'vitest/config';

export default defineConfig({
  base: './',
  plugins: [react()],
  root: '.',
  build: {
    outDir: 'dist/renderer',
    emptyOutDir: true,
  },
  server: {
    host: '127.0.0.1',
    port: 5173,
  },
  test: {
    exclude: [...defaultExclude, 'tests/e2e/**'],
    passWithNoTests: true,
  },
});
