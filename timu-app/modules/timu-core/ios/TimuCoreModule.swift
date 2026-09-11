// Expo module wrapping the timu-core UniFFI bindings (ADR-012).
//
// The Rust side owns SSH/tmux/store; this layer only converts types and
// bridges async Rust calls onto Expo Promises and pane-stream callbacks onto
// module events. Generated bindings (`Generated/timu_core.swift`) compile into
// this pod; the Rust static lib ships as Frameworks/TimuCoreFFI.xcframework
// (built by scripts/build-core.sh).

import ExpoModulesCore
import Foundation

// MARK: - JS input records

struct ProfileArgs: Record {
  @Field var name: String = ""
  @Field var host: String = ""
  @Field var username: String = ""
  @Field var port: Int = 22
  @Field var authMethod: String = "Password"
}

struct CredentialsArgs: Record {
  /// "Password" or "Key".
  @Field var kind: String = "Password"
  @Field var password: String = ""
  /// Private key text (PEM / OpenSSH) for kind = "Key".
  @Field var material: String = ""
  @Field var passphrase: String? = nil
}

struct FolderEntryArgs: Record {
  @Field var path: String = ""
  @Field var name: String = ""
  @Field var isGitRepo: Bool = false
}

struct SessionArgs: Record {
  @Field var id: Int64 = 0
  @Field var profileId: Int64 = 0
  @Field var agent: String = ""
  @Field var folder: String = ""
  @Field var tmuxSessionId: String = ""
  @Field var status: String = "active"
}

struct HostKeyPinArgs: Record {
  @Field var host: String = ""
  @Field var fingerprint: String = ""
}

// MARK: - Conversions

private func authMethod(from name: String) throws -> AuthMethod {
  switch name {
  case "Password": return .password
  case "KeyPaste": return .keyPaste
  case "KeyFile": return .keyFile
  default: throw TimuCoreInvalidInput("Unknown authMethod: \(name)")
  }
}

private func authMethodName(_ method: AuthMethod) -> String {
  switch method {
  case .password: return "Password"
  case .keyPaste: return "KeyPaste"
  case .keyFile: return "KeyFile"
  }
}

private func credentials(from args: CredentialsArgs) throws -> Credentials {
  switch args.kind {
  case "Password": return .password(args.password)
  case "Key": return .privateKey(material: Data(args.material.utf8), passphrase: args.passphrase)
  default: throw TimuCoreInvalidInput("Unknown credentials kind: \(args.kind)")
  }
}

private func machineProfile(from args: ProfileArgs) throws -> MachineProfile {
  MachineProfile(
    name: args.name,
    host: args.host,
    username: args.username,
    port: UInt16(max(0, min(Int(UInt16.max), args.port))),
    authMethod: try authMethod(from: args.authMethod)
  )
}

private func folderEntry(from args: FolderEntryArgs) -> FolderEntry {
  FolderEntry(path: args.path, name: args.name, isGitRepo: args.isGitRepo)
}

private func sessionRecord(from args: SessionArgs) -> SessionRecord {
  SessionRecord(
    id: args.id,
    profileId: args.profileId,
    agent: args.agent,
    folder: args.folder,
    tmuxSessionId: args.tmuxSessionId,
    status: args.status
  )
}

// MARK: - TimuError → JS shape

private func errorDict(_ error: TimuError) -> [String: Any?] {
  switch error {
  case .WrongHost:
    return ["code": "wrong_host", "userLabel": "Couldn't reach that host"]
  case .WrongUsername:
    return ["code": "wrong_username", "userLabel": "That username was rejected"]
  case .WrongCredentials:
    return ["code": "wrong_credentials", "userLabel": "Wrong password or key"]
  case .PortUnreachable:
    return ["code": "port_unreachable", "userLabel": "That port isn't reachable"]
  case .NetworkUnavailable:
    return ["code": "network_unavailable", "userLabel": "No network connection"]
  case .PermissionDenied:
    return ["code": "permission_denied", "userLabel": "Not allowed to log in"]
  case .TmuxMissing:
    return ["code": "tmux_missing", "userLabel": "tmux isn't installed on that machine"]
  case .Other(let message):
    return ["code": "other", "userLabel": "Something went wrong", "message": message]
  }
}

/// Reject with the stable PRD §6 error code so JS can branch on `code`.
private func rejectTimu(_ promise: Promise, _ error: Error) {
  if let timuError = error as? TimuError {
    let dict = errorDict(timuError)
    promise.reject("TIMU_\(dict["code"] as? String ?? "other")", dict["userLabel"] as? String ?? "Something went wrong")
  } else {
    promise.reject("TIMU_unexpected", error.localizedDescription)
  }
}

private func outcomeDict(_ outcome: ConnectionTestOutcome) -> [String: Any?] {
  switch outcome {
  case .connected(let fingerprint):
    return ["kind": "Connected", "fingerprint": fingerprint]
  case .failed(let error):
    var dict = errorDict(error)
    dict["kind"] = "Failed"
    return dict
  }
}

private func readinessDict(_ report: ReadinessReport) -> [String: Any?] {
  let tools: [(Tool, String)] = [
    (.tmux, "tmux"), (.git, "git"), (.shell, "shell"), (.node, "node"),
    (.npm, "npm"), (.pnpm, "pnpm"), (.yarn, "yarn"), (.bun, "bun"),
    (.codex, "codex"), (.claude, "claude"), (.openCode, "openCode"),
  ]
  var statuses: [String: String] = [:]
  for (tool, name) in tools {
    let status: String
    switch report.get(tool: tool) {
    case .ready: status = "Ready"
    case .missing: status = "Missing"
    case .unknown: status = "Unknown"
    }
    statuses[name] = status
  }
  return ["statuses": statuses, "tmuxMissing": report.tmuxIsMissing(), "rendered": report.render()]
}

private func folderDict(_ entry: FolderEntry) -> [String: Any?] {
  ["path": entry.path, "name": entry.name, "isGitRepo": entry.isGitRepo]
}

private func profileDict(_ record: ProfileRecord) -> [String: Any?] {
  [
    "id": record.id, "name": record.name, "host": record.host,
    "username": record.username, "port": Int(record.port),
    "authMethod": authMethodName(record.authMethod),
  ]
}

private func sessionDict(_ record: SessionRecord) -> [String: Any?] {
  [
    "id": record.id, "profileId": record.profileId, "agent": record.agent,
    "folder": record.folder, "tmuxSessionId": record.tmuxSessionId, "status": record.status,
  ]
}

private func pinDict(_ pin: HostKeyPin) -> [String: Any?] {
  ["host": pin.host, "fingerprint": pin.fingerprint]
}

private func paneEventDict(_ event: PaneEvent) -> [String: Any?] {
  switch event {
  case .history(let text): return ["kind": "History", "text": text]
  case .outputAppended(let text): return ["kind": "OutputAppended", "text": text]
  case .sessionEnded: return ["kind": "SessionEnded"]
  }
}

// MARK: - Shared objects

final class ExpoConnection: SharedObject {
  let raw: Connection
  init(raw: Connection) {
    self.raw = raw
    super.init()
  }
}

final class ExpoPaneStreamHandle: SharedObject {
  let raw: PaneStreamHandle
  init(raw: PaneStreamHandle) {
    self.raw = raw
    super.init()
  }
}

final class ExpoStore: SharedObject {
  let raw: Store
  init(raw: Store) {
    self.raw = raw
    super.init()
  }
}

/// Bridges Rust pane-stream callbacks onto module events, tagged per session.
final class PaneEventBridge: PaneEventSink {
  private weak var module: TimuCoreModule?
  private let sessionId: String

  init(module: TimuCoreModule, sessionId: String) {
    self.module = module
    self.sessionId = sessionId
  }

  func onEvent(event: PaneEvent) {
    module?.sendEvent("onPaneEvent", ["sessionId": sessionId, "event": paneEventDict(event)])
  }

  func onError(message: String) {
    module?.sendEvent("onPaneError", ["sessionId": sessionId, "message": message])
  }
}

// MARK: - Typed exceptions for malformed JS input

final class TimuCoreInvalidInput: Exception {
  private let message: String

  init(_ message: String) {
    self.message = message
    super.init()
  }

  override var reason: String { message }
}

// MARK: - Module

public final class TimuCoreModule: Module {
  private var core: FfiCore?

  /// Lazily-created shared Rust core (host-key pins live here).
  private func sharedCore() -> FfiCore {
    if let core { return core }
    let created = FfiCore()
    core = created
    return created
  }

  public func definition() -> ModuleDefinition {
    Name("TimuCore")
    Events("onPaneEvent", "onPaneError")

    // PRD §6 — connection test. Never rejects: failures are data.
    AsyncFunction("testConnection") { (profile: ProfileArgs, creds: CredentialsArgs, promise: Promise) in
      let core = sharedCore()
      let p = try machineProfile(from: profile)
      let c = try credentials(from: creds)
      Task {
        let outcome = await core.testConnection(profile: p, creds: c)
        promise.resolve(outcomeDict(outcome))
      }
    }

    /// Open + authenticate a live connection for session work.
    AsyncFunction("connect") { (profile: ProfileArgs, creds: CredentialsArgs, promise: Promise) in
      let core = sharedCore()
      let p = try machineProfile(from: profile)
      let c = try credentials(from: creds)
      Task {
        do {
          let connection = try await core.connect(profile: p, creds: c)
          promise.resolve(ExpoConnection(raw: connection))
        } catch {
          rejectTimu(promise, error)
        }
      }
    }

    /// Inject pins loaded from the SQLite store at app boot (no re-TOFU).
    AsyncFunction("loadPins") { (pins: [HostKeyPinArgs], promise: Promise) in
      let core = sharedCore()
      let mapped = pins.map { HostKeyPin(host: $0.host, fingerprint: $0.fingerprint) }
      Task {
        await core.loadPins(pins: mapped)
        promise.resolve(nil)
      }
    }

    /// Current in-memory pins, for persisting new ones after a connect.
    AsyncFunction("getPins") { (promise: Promise) in
      let core = sharedCore()
      Task {
        let pins = await core.pins()
        promise.resolve(pins.map(pinDict))
      }
    }

    /// SQLite store (profiles, sessions, folders, pins). Sync: local SQLite.
    Function("openStore") { (path: String) -> ExpoStore in
      ExpoStore(raw: try Store.open(path: path))
    }

    Class("Connection", ExpoConnection.self) {
      AsyncFunction("readiness") { (connection: ExpoConnection, promise: Promise) in
        Task {
          do {
            promise.resolve(readinessDict(try await connection.raw.readiness()))
          } catch {
            rejectTimu(promise, error)
          }
        }
      }

      AsyncFunction("startAgentSession") { (connection: ExpoConnection, folder: String, agentCommand: String, promise: Promise) in
        Task {
          do {
            let started = try await connection.raw.startAgentSession(folder: folder, agentCommand: agentCommand)
            promise.resolve(["sessionId": started.sessionId, "reused": started.reused])
          } catch {
            rejectTimu(promise, error)
          }
        }
      }

      AsyncFunction("sendChatMessage") { (connection: ExpoConnection, sessionId: String, text: String, promise: Promise) in
        Task {
          do {
            try await connection.raw.sendChatMessage(sessionId: sessionId, text: text)
            promise.resolve(nil)
          } catch {
            rejectTimu(promise, error)
          }
        }
      }

      AsyncFunction("capturePane") { (connection: ExpoConnection, sessionId: String, promise: Promise) in
        Task {
          do {
            promise.resolve(try await connection.raw.capturePane(sessionId: sessionId))
          } catch {
            rejectTimu(promise, error)
          }
        }
      }

      AsyncFunction("listTmuxSessions") { (connection: ExpoConnection, promise: Promise) in
        Task {
          do {
            promise.resolve(try await connection.raw.listTmuxSessions())
          } catch {
            rejectTimu(promise, error)
          }
        }
      }

      AsyncFunction("killSession") { (connection: ExpoConnection, sessionId: String, promise: Promise) in
        Task {
          do {
            try await connection.raw.killSession(sessionId: sessionId)
            promise.resolve(nil)
          } catch {
            rejectTimu(promise, error)
          }
        }
      }

      /// Emits `onPaneEvent` / `onPaneError` module events tagged with sessionId.
      AsyncFunction("startPaneStream") { (connection: ExpoConnection, sessionId: String, intervalMs: Double, promise: Promise) in
        let sink = PaneEventBridge(module: self, sessionId: sessionId)
        Task {
          do {
            let handle = try await connection.raw.startPaneStream(
              sessionId: sessionId,
              intervalMs: UInt64(max(50, intervalMs)),
              sink: sink
            )
            promise.resolve(ExpoPaneStreamHandle(raw: handle))
          } catch {
            rejectTimu(promise, error)
          }
        }
      }

      AsyncFunction("disconnect") { (connection: ExpoConnection, promise: Promise) in
        Task {
          await connection.raw.disconnect()
          promise.resolve(nil)
        }
      }
    }

    Class("PaneStreamHandle", ExpoPaneStreamHandle.self) {
      Function("stop") { (handle: ExpoPaneStreamHandle) in
        handle.raw.stop()
      }
    }

    Class("Store", ExpoStore.self) {
      Function("saveProfile") { (store: ExpoStore, profile: ProfileArgs) -> Int64 in
        try store.raw.saveProfile(profile: try machineProfile(from: profile))
      }

      Function("listProfiles") { (store: ExpoStore) -> [[String: Any?]] in
        try store.raw.listProfiles().map(profileDict)
      }

      Function("getProfile") { (store: ExpoStore, id: Int64) -> [String: Any?]? in
        try store.raw.getProfile(id: id).map(profileDict)
      }

      Function("deleteProfile") { (store: ExpoStore, id: Int64) -> Bool in
        try store.raw.deleteProfile(id: id)
      }

      Function("touchProfile") { (store: ExpoStore, id: Int64) in
        try store.raw.touchProfile(id: id)
      }

      /// `id = 0` inserts; a non-zero id updates in place.
      Function("saveSession") { (store: ExpoStore, session: SessionArgs) -> Int64 in
        try store.raw.saveSession(session: sessionRecord(from: session))
      }

      Function("listSessions") { (store: ExpoStore, profileId: Int64) -> [[String: Any?]] in
        try store.raw.listSessions(profileId: profileId).map(sessionDict)
      }

      Function("addRecentFolder") { (store: ExpoStore, entry: FolderEntryArgs) in
        try store.raw.addRecentFolder(entry: folderEntry(from: entry))
      }

      Function("listRecentFolders") { (store: ExpoStore) -> [[String: Any?]] in
        try store.raw.listRecentFolders().map(folderDict)
      }

      Function("addFavorite") { (store: ExpoStore, entry: FolderEntryArgs) in
        try store.raw.addFavorite(entry: folderEntry(from: entry))
      }

      Function("listFavorites") { (store: ExpoStore) -> [[String: Any?]] in
        try store.raw.listFavorites().map(folderDict)
      }

      Function("removeFavorite") { (store: ExpoStore, path: String) -> Bool in
        try store.raw.removeFavorite(path: path)
      }

      Function("saveHostKeyPin") { (store: ExpoStore, host: String, fingerprint: String) in
        try store.raw.saveHostKeyPin(host: host, fingerprint: fingerprint)
      }

      Function("loadHostKeyPins") { (store: ExpoStore) -> [[String: Any?]] in
        try store.raw.loadHostKeyPins().map(pinDict)
      }
    }
  }
}