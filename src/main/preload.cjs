const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('als', {
  version: '0.1.0',
  getEngineUrl: () => ipcRenderer.invoke('engine:get-url'),
});
