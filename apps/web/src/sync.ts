// Real-time document sync with the offkilter server.
//
// The client applies its own ops optimistically and sends them; the server
// applies each op to its copy, numbers it, and broadcasts it. Ops from other
// clients are applied on arrival. If another client's op arrives while our
// own ops are still unacknowledged, the two sides may have applied them in
// different orders, so we resync from a server snapshot.

import type { DocOp } from "./kernel";

export type UserInfo = { id: string; name: string };
export type DocMeta = { id: string; name: string; created: number; updated: number; owner?: UserInfo; collaborators?: UserInfo[]; viewers?: UserInfo[] };
export type VersionMeta = { id: string; name: string; created: number };

type ServerMessage =
  | { type: "welcome"; client: number; prefix: number; seq: number; doc: string; clients: number; read_only?: boolean }
  | { type: "op"; op: DocOp; seq: number; from: number; id: number; base: number | null; hash: string }
  | { type: "error"; message: string; id: number }
  | { type: "snapshot"; doc: string; seq: number }
  | { type: "presence"; clients: number; names: string[] };

export interface SyncHandlers {
  /** Apply an op that came from another client, allocating ids from `base`. */
  remoteOp(op: DocOp, base: number | null): void;
  /** Current structural hash of the local document. */
  localHash(): string;
  /** Replace the whole document (welcome, resync, or a remote undo). */
  loadDocument(json: string): void;
  presence(clients: number, names: string[]): void;
  status(text: string): void;
  /** The server shared this document with the current account read-only (or not). */
  readOnly(readOnly: boolean): void;
}

export class Sync {
  private ws: WebSocket | null = null;
  private nextId = 1;
  private inflight = new Set<number>();
  private clientId = 0;
  /** Id-range prefix from the server; ids are (prefix << 20) | counter. */
  private prefix = 0;
  private counter = 0;
  /** Ids allocated per op; ops allocate far fewer than this. */
  private static readonly STRIDE = 256;
  docId: string | null = null;
  connected = false;
  /** Snapshot resyncs so far (a diverged replica or a rejected op); tests expect none. */
  resyncs = 0;

  constructor(private handlers: SyncHandlers, private userName: string) {}

  static apiBase(): string {
    return `${location.origin}/api`;
  }

  /** Error text from a failed response: the body when the server explains, else the status. */
  private static async failure(r: Response): Promise<Error> {
    const text = (await r.text()).trim();
    return new Error(text && text.length < 200 ? text : `server said ${r.status}`);
  }

  // ---- accounts (cookie sessions; same-origin fetches carry the cookie)

  static async me(): Promise<UserInfo | null> {
    try {
      const r = await fetch(`${Sync.apiBase()}/auth/me`);
      return r.ok ? ((await r.json()) as UserInfo) : null;
    } catch {
      return null;
    }
  }

  static async register(name: string, password: string): Promise<UserInfo> {
    const r = await fetch(`${Sync.apiBase()}/auth/register`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ name, password }) });
    if (!r.ok) throw await Sync.failure(r);
    return (await r.json()) as UserInfo;
  }

  static async login(name: string, password: string): Promise<UserInfo> {
    const r = await fetch(`${Sync.apiBase()}/auth/login`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ name, password }) });
    if (!r.ok) throw await Sync.failure(r);
    return (await r.json()) as UserInfo;
  }

  static async logout(): Promise<void> {
    await fetch(`${Sync.apiBase()}/auth/logout`, { method: "POST" });
  }

  static async shareDoc(id: string, name: string, role: "editor" | "viewer" = "editor"): Promise<DocMeta> {
    const r = await fetch(`${Sync.apiBase()}/docs/${id}/share`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ name, role }) });
    if (!r.ok) throw await Sync.failure(r);
    return (await r.json()) as DocMeta;
  }

  static async unshareDoc(id: string, userId: string): Promise<DocMeta> {
    const r = await fetch(`${Sync.apiBase()}/docs/${id}/share/${userId}`, { method: "DELETE" });
    if (!r.ok) throw await Sync.failure(r);
    return (await r.json()) as DocMeta;
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
    const r = await fetch(`${Sync.apiBase()}/docs/${id}`, { method: "DELETE" });
    if (!r.ok) throw await Sync.failure(r);
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

  /**
   * A fresh id base for the next local op, from this client's range, or
   * null when not connected (the document's own counters are used).
   */
  nextBase(): number | null {
    if (!this.connected) return null;
    this.counter += Sync.STRIDE;
    return (this.prefix << 20) | this.counter;
  }

  /** Sends an op this client already applied locally with `base`. */
  send(op: DocOp, base: number | null): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    const id = this.nextId++;
    this.inflight.add(id);
    this.ws.send(JSON.stringify({ type: "op", op, id, base }));
  }

  private requestSnapshot(): void {
    this.resyncs++;
    this.inflight.clear();
    this.ws?.send(JSON.stringify({ type: "snapshot" }));
  }

  private onMessage(m: ServerMessage): void {
    switch (m.type) {
      case "welcome":
        this.clientId = m.client;
        this.prefix = m.prefix;
        this.counter = 0;
        this.connected = true;
        this.handlers.readOnly(!!m.read_only);
        this.handlers.loadDocument(m.doc);
        this.handlers.presence(m.clients, []);
        this.handlers.status(`connected · ${m.clients} online`);
        break;
      case "op":
        if (m.from === this.clientId) {
          this.inflight.delete(m.id);
        } else if (m.op.type === "replace_document") {
          this.handlers.loadDocument(m.op.json);
          return;
        } else {
          // Ids come from the sender's range, so applying out of order is safe.
          this.handlers.remoteOp(m.op, m.base);
        }
        // With nothing of ours in flight, our document must match the server's.
        if (this.inflight.size === 0 && this.handlers.localHash() !== m.hash) {
          this.handlers.status("replica diverged; resyncing");
          this.requestSnapshot();
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
