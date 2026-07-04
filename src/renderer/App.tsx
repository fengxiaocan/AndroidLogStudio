import { useEffect, useRef } from 'react';
import { EngineClient } from './api/engineClient';
import { useAppStore } from './state/appStore';

export function App() {
  const connected = useAppStore((state) => state.connected);
  const logs = useAppStore((state) => state.logs);
  const handleServerMessage = useAppStore((state) => state.handleServerMessage);
  const clientRef = useRef<EngineClient | null>(null);
  const hasConnectedRef = useRef(false);

  useEffect(() => {
    if (hasConnectedRef.current) {
      return;
    }

    hasConnectedRef.current = true;
    clientRef.current = new EngineClient(handleServerMessage);
    void clientRef.current.connect();
  }, [handleServerMessage]);

  return (
    <main className="app-shell">
      <header className="toolbar">Android Logcat Studio</header>
      <section className="empty-state">
        {connected ? `Connected: ${logs.length} visible logs` : 'Connecting to engine...'}
      </section>
    </main>
  );
}
