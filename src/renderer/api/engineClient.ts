import type { ClientMessage, ServerMessage } from '../types/protocol';

export type ServerMessageHandler = (message: ServerMessage) => void;

export class EngineClient {
  private socket: WebSocket | null = null;

  constructor(private readonly onMessage: ServerMessageHandler) {}

  async connect(): Promise<void> {
    if (this.socket?.readyState === WebSocket.OPEN || this.socket?.readyState === WebSocket.CONNECTING) {
      return;
    }

    const engineUrl = await window.als.getEngineUrl();
    const socket = new WebSocket(engineUrl);
    this.socket = socket;

    socket.onmessage = (event) => {
      const message = JSON.parse(String(event.data)) as ServerMessage;
      this.onMessage(message);
    };
  }

  send(message: ClientMessage): boolean {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      return false;
    }

    this.socket.send(JSON.stringify(message));
    return true;
  }
}
