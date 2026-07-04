import { contextBridge } from 'electron';

contextBridge.exposeInMainWorld('als', {
  version: '0.1.0',
});
