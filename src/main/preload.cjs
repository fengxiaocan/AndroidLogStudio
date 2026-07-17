const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('als', {
  version: '1.0.0',
  getEngineUrl: () => ipcRenderer.invoke('engine:get-url'),
  exportSave: (tempPath, defaultName) =>
    ipcRenderer.invoke('export:save', { tempPath, defaultName }),
});
