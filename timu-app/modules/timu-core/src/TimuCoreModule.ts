// Thin typed adapter over the native TimuCore module. All Rust/UniFFI
// specifics (TIMU_ error codes, event tagging, promise shapes) stay here.
//
// Native shape: Connection / PaneStreamHandle / Store come back as shared
// objects with their class methods bound; pane streaming arrives as module
// events tagged with the tmux session id.

import { requireNativeModule } from "expo-modules-core";

export type AuthMethod = "Password" | "KeyPaste" | "KeyFile";

export interface MachineProfile {
  name: string;
  host: string;
  username: string;
  port: number;
  authMethod: AuthMethod;
}

export type Credentials =
  | { kind: "Password"; password: string }
  | { kind: "Key"; material: string; passphrase?: string };

export interface TimuErrorShape {
  code: string;
  userLabel: string;
  message?: string;
}

export type ConnectionTestOutcome =
  | { kind: "Connected"; fingerprint?: string | null }
  | ({ kind: "Failed" } & TimuErrorShape);

export interface ReadinessReport {
  statuses: Record<string, "Ready" | "Missing" | "Unknown">;
  tmuxMissing: boolean;
  rendered: string;
}

export interface StartedSession {
  sessionId: string;
  reused: boolean;
}

export interface HostKeyPin {
  host: string;
  fingerprint: string;
}

export interface FolderEntry {
  path: string;
  name: string;
  isGitRepo: boolean;
}

export interface SessionRecord {
  id: number;
  profileId: number;
  agent: string;
  folder: string;
  tmuxSessionId: string;
  status: string;
}

export interface ProfileRecord {
  id: number;
  name: string;
  host: string;
  username: string;
  port: number;
  authMethod: AuthMethod;
}

/** Native pane event, tagged with the tmux session it belongs to. */
export interface PaneEventPayload {
  sessionId: string;
  event: { kind: "History" | "OutputAppended"; text: string } | { kind: "SessionEnded" };
}

/** Re-thrown native failure with the stable PRD §6 code (TIMU_ prefix stripped). */
export class TimuNativeError extends Error {
  readonly code: string;
  readonly userLabel: string;

  constructor(code: string, userLabel: string) {
    super(userLabel);
    this.name = "TimuNativeError";
    this.code = code;
    this.userLabel = userLabel;
  }
}

function rethrow(error: unknown): never {
  const code = (error as { code?: string })?.code ?? "unexpected";
  const message = (error as { message?: string })?.message;
  throw new TimuNativeError(
    code.replace(/^TIMU_/, ""),
    message && message !== "timu-core error" ? message : "Something went wrong"
  );
}

function credsPayload(creds: Credentials) {
  return creds.kind === "Password"
    ? { kind: "Password", password: creds.password, material: "", passphrase: undefined }
    : { kind: "Key", password: "", material: creds.material, passphrase: creds.passphrase };
}

// Shared-object surfaces, exactly as defined by the native Class() blocks.
type NativeConnectionObject = {
  readiness(): Promise<ReadinessReport>;
  startAgentSession(folder: string, agentCommand: string): Promise<StartedSession>;
  sendChatMessage(sessionId: string, text: string): Promise<void>;
  capturePane(sessionId: string): Promise<string>;
  listTmuxSessions(): Promise<string[]>;
  killSession(sessionId: string): Promise<void>;
  startPaneStream(sessionId: string, intervalMs: number): Promise<NativePaneStreamHandle>;
  disconnect(): Promise<void>;
};

type NativePaneStreamHandle = {
  stop(): void;
};

type NativeStoreObject = {
  saveProfile(profile: MachineProfile): Promise<number>;
  listProfiles(): Promise<ProfileRecord[]>;
  getProfile(id: number): Promise<ProfileRecord | null>;
  deleteProfile(id: number): Promise<boolean>;
  touchProfile(id: number): Promise<void>;
  saveSession(session: SessionRecord): Promise<number>;
  listSessions(profileId: number): Promise<SessionRecord[]>;
  addRecentFolder(entry: FolderEntry): Promise<void>;
  listRecentFolders(): Promise<FolderEntry[]>;
  addFavorite(entry: FolderEntry): Promise<void>;
  listFavorites(): Promise<FolderEntry[]>;
  removeFavorite(path: string): Promise<boolean>;
  saveHostKeyPin(host: string, fingerprint: string): Promise<void>;
  loadHostKeyPins(): Promise<HostKeyPin[]>;
};

type NativeTimuCore = {
  testConnection(
    profile: MachineProfile,
    creds: { kind: string; password: string; material: string; passphrase?: string }
  ): Promise<ConnectionTestOutcome>;
  connect(
    profile: MachineProfile,
    creds: { kind: string; password: string; material: string; passphrase?: string }
  ): Promise<NativeConnectionObject>;
  loadPins(pins: HostKeyPin[]): Promise<void>;
  getPins(): Promise<HostKeyPin[]>;
  openStore(path: string): Promise<NativeStoreObject>;
};

export class NativeConnection {
  constructor(
    private readonly raw: NativeConnectionObject,
    private readonly events: { addListener< T >(
      event: "onPaneEvent" | "onPaneError",
      listener: (payload: never) => void
    ): { remove(): void } }
  ) {}

  async readiness(): Promise<ReadinessReport> {
    return this.raw.readiness();
  }

  async startAgentSession(folder: string, agentCommand: string): Promise<StartedSession> {
    return this.raw.startAgentSession(folder, agentCommand);
  }

  async sendChatMessage(sessionId: string, text: string): Promise<void> {
    await this.raw.sendChatMessage(sessionId, text);
  }

  async capturePane(sessionId: string): Promise<string> {
    return this.raw.capturePane(sessionId);
  }

  async listTmuxSessions(): Promise<string[]> {
    return this.raw.listTmuxSessions();
  }

  async killSession(sessionId: string): Promise<void> {
    await this.raw.killSession(sessionId);
  }

  /** Subscribes to this session's pane events; `stop()` cancels the watcher. */
  async startPaneStream(
    sessionId: string,
    onEvent: (payload: PaneEventPayload) => void,
    onError: (message: string) => void,
    intervalMs = 250
  ): Promise<PaneStreamHandle> {
    const paneSubscription = this.events.addListener<PaneEventPayload>(
      "onPaneEvent" as never,
      (payload: PaneEventPayload) => {
        if (payload.sessionId === sessionId) onEvent(payload);
      }
    );
    const errorSubscription = this.events.addListener<{ sessionId: string; message: string }>(
      "onPaneError" as never,
      (payload: { sessionId: string; message: string }) => {
        if (payload.sessionId === sessionId) onError(payload.message);
      }
    );
    try {
      const handle = await this.raw.startPaneStream(sessionId, intervalMs);
      return new PaneStreamHandle(handle, [paneSubscription, errorSubscription]);
    } catch (error) {
      paneSubscription.remove();
      errorSubscription.remove();
      rethrow(error);
    }
  }

  async disconnect(): Promise<void> {
    await this.raw.disconnect();
  }
}

export class PaneStreamHandle {
  constructor(
    private readonly raw: NativePaneStreamHandle,
    private readonly subscriptions: { remove(): void }[]
  ) {}

  stop(): void {
    this.raw.stop();
    for (const subscription of this.subscriptions) subscription.remove();
  }
}

export class NativeStore {
  constructor(private readonly raw: NativeStoreObject) {}

  async saveProfile(profile: MachineProfile): Promise<number> {
    return this.raw.saveProfile(profile);
  }
  async listProfiles(): Promise<ProfileRecord[]> {
    return this.raw.listProfiles();
  }
  async getProfile(id: number): Promise<ProfileRecord | null> {
    return this.raw.getProfile(id);
  }
  async deleteProfile(id: number): Promise<boolean> {
    return this.raw.deleteProfile(id);
  }
  async touchProfile(id: number): Promise<void> {
    await this.raw.touchProfile(id);
  }
  async saveSession(session: SessionRecord): Promise<number> {
    return this.raw.saveSession(session);
  }
  async listSessions(profileId: number): Promise<SessionRecord[]> {
    return this.raw.listSessions(profileId);
  }
  async addRecentFolder(entry: FolderEntry): Promise<void> {
    await this.raw.addRecentFolder(entry);
  }
  async listRecentFolders(): Promise<FolderEntry[]> {
    return this.raw.listRecentFolders();
  }
  async addFavorite(entry: FolderEntry): Promise<void> {
    await this.raw.addFavorite(entry);
  }
  async listFavorites(): Promise<FolderEntry[]> {
    return this.raw.listFavorites();
  }
  async removeFavorite(path: string): Promise<boolean> {
    return this.raw.removeFavorite(path);
  }
  async saveHostKeyPin(host: string, fingerprint: string): Promise<void> {
    await this.raw.saveHostKeyPin(host, fingerprint);
  }
  async loadHostKeyPins(): Promise<HostKeyPin[]> {
    return this.raw.loadHostKeyPins();
  }
}

/**
 * App-facing facade over timu-core. Pins are shared by the native layer.
 */
export const TimuCore = {
  async testConnection(
    profile: MachineProfile,
    creds: Credentials
  ): Promise<ConnectionTestOutcome> {
    try {
      return await native.testConnection(profile, credsPayload(creds));
    } catch (error) {
      rethrow(error);
    }
  },

  async connect(profile: MachineProfile, creds: Credentials): Promise<NativeConnection> {
    try {
      return new NativeConnection(await native.connect(profile, credsPayload(creds)), events);
    } catch (error) {
      rethrow(error);
    }
  },

  /** Current pins (call after a connect to persist any new TOFU pin). */
  async getPins(): Promise<HostKeyPin[]> {
    try {
      return await native.getPins();
    } catch (error) {
      rethrow(error);
    }
  },

  /** Inject pins at boot so the first connect verifies instead of re-TOFU. */
  async loadPins(pins: HostKeyPin[]): Promise<void> {
    try {
      await native.loadPins(pins);
    } catch (error) {
      rethrow(error);
    }
  },

  /** Opens the SQLite store; creates the file if missing. */
  async openStore(path: string): Promise<NativeStore> {
    try {
      return new NativeStore(await native.openStore(path));
    } catch (error) {
      rethrow(error);
    }
  },
};

const native = requireNativeModule<NativeTimuCore>("TimuCore");
const events = requireNativeModule("TimuCore") as unknown as {
  addListener<T>(eventName: string, listener: (payload: T) => void): { remove(): void };
};