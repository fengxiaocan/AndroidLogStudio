// Preload is intentionally CommonJS: see preload.cjs.
// package.json has "type":"module", so a .js preload is treated as ESM and
// fails under Electron's preload sandbox, leaving window.als undefined.
// The build copies src/main/preload.cjs → dist/main/preload.cjs.
export {};
