// Real-time document sync with the offkilter server.
//
// The client applies its own ops optimistically and sends them; the server
// applies each op to its copy, numbers it, and broadcasts it. Ops from other
// clients are applied on arrival. If another client's op arrives while our
// own ops are still unacknowledged, the two sides may have applied them in
// different orders, so we resync from a server snapshot.

import type { Op } from "./kernel";

export type DocMeta = { id: string; name: string; created: number; updated: number };
export type VersionMeta = { id: string; name: string; created: number };

type ServerMessage =
  | { type: "welcome"; client: number; seq: number; doc: string; clients: number }
  | { type: "op"; op: Op; seq: number; from: number; id: number }
  | { type: "error"; message: string; id: number }
  | { type: "snapshot"; doc: string; seq: number }
  | { type: "presence"; clients: number; names: string[] };

export interface SyncHandlers {
  /** Apply an op that came from another client. */
  remoteOp(op: Op): void;
  /** Replace the whole document (welcome, resync, or a remote undo). */
  loadDocument(json: string): void;
  presence(clients: number, names: string[]): void;
  status(text: string): void;
}

export class Sync {
  private ws: WebSocket | null = null;
  private nextId = 1;
  private inflight = new Set<number>();
  private clientId = 0;
  docId: string | null = null;
  connected = false;

  constructor(private handlers: SyncHandlers, private userName: string) {}

  static apiBase(): string {
    return `${location.origin}/api`;
  }

  static async listDocs(): Promise<DocMeta[]> {
    const r = await fetch(`${Sync.apiBase()}/docs`);
    if (!r.ok) throw new Error(`server said ${r.status}`);
    return (await r.json()) as DocMeta[];
  }

  static async createDoc(name: string, json?: string): Promise<DocMeta> {
    const r = await fetch(`${Sync.apiBase()}/docs`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ name, json }) });
    if (!r.ok) throw new Error(`server said ${r.status}`);
    return (await r.json()) as DocMeta;
  }

  static async deleteDoc(id: string): Promise<void> {
    await fetch(`${Sync.apiBase()}/docs/${id}`, { method: "DELETE" });
  }

  static async listVersions(id: string): Promise<VersionMeta[]> {
    const r = await fetch(`${Sync.apiBase()}/docs/${id}/versions`);
    if (!r.ok) throw new Error(`server said ${r.status}`);
    return (await r.json()) as VersionMeta[];
  }

  static async saveVersion(id: string, name: string): Promise<VersionMeta> {
    const r = await fetch(`${Sync.apiBase()}/docs/${id}/versions`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ name }) });
    if (!r.ok) throw new Error(`server said ${r.status}`);
    return (await r.json()) as VersionMeta;
  }

  /** Restores a version; every connected client reloads the document. */
  static async restoreVersion(id: string, vid: string): Promise<void> {
    const r = await fetch(`${Sync.apiBase()}/docs/${id}/versions/${vid}/restore`, { method: "POST" });
    if (!r.ok) throw new Error(`server said ${r.status}`);
  }

  /** Whether a server is reachable at this origin. */
  static async available(): Promise<boolean> {
    try {
      const r = await fetch(`${Sync.apiBase()}/health`);
      return r.ok;
    } catch {
      return false;
    }
  }

  connect(docId: string): void {
    this.disconnect();
    this.docId = docId;
    const proto = location.protocol === "https:" ? "wss" : "ws";
    const ws = new WebSocket(`${proto}://${location.host}/api/docs/${docId}/ws`);
    this.ws = ws;
    ws.onopen = () => {
      ws.send(JSON.stringify({ type: "hello", name: this.userName }));
    };
    ws.onmessage = (ev) => this.onMessage(JSON.parse(ev.data as string) as ServerMessage);
    ws.onclose = () => {
      this.connected = false;
      this.handlers.status("disconnected from server");
    };
    ws.onerror = () => this.handlers.status("connection error");
  }

  disconnect(): void {
    this.ws?.close();
    this.ws = null;
    this.docId = null;
    this.connected = false;
    this.inflight.clear();
  }

  /** Sends an op this client already applied locally. */
  send(op: Op): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    const id = this.nextId++;
    this.inflight.add(id);
    this.ws.send(JSON.stringify({ type: "op", op, id }));
  }

  private requestSnapshot(): void {
    this.inflight.clear();
    this.ws?.send(JSON.stringify({ type: "snapshot" }));
  }

  private onMessage(m: ServerMessage): void {
    switch (m.type) {
      case "welcome":
        this.clientId = m.client;
        this.connected = true;
        this.handlers.loadDocument(m.doc);
        this.handlers.presence(m.clients, []);
        this.handlers.status(`connected · ${m.clients} online`);
        break;
      case "op":
        if (m.from === this.clientId) {
          this.inflight.delete(m.id);
        } else if (this.inflight.size > 0) {
          // Concurrent edits: server order may differ from ours; resync.
          this.requestSnapshot();
        } else if (m.op.type === "replace_document") {
          this.handlers.loadDocument(m.op.json);
        } else {
          this.handlers.remoteOp(m.op);
        }
        break;
      case "error":
        this.handlers.status(`server rejected an edit: ${m.message}`);
        this.requestSnapshot();
        break;
      case "snapshot":
        this.handlers.loadDocument(m.doc);
        break;
      case "presence":
        this.handlers.presence(m.clients, m.names);
        break;
    }
  }
}
