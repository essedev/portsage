export interface PortStatus {
  id: number;
  project_id: number;
  service: string;
  port: number;
  active: boolean;
  process: string | null;
  pid: number | null;
  created_at: string;
}

export type KillOutcome =
  | "terminated"
  | "killed"
  | "not_active"
  | "permission_denied";

export interface UnmanagedPort {
  port: number;
  process: string;
  pid: number;
}

export interface ProjectStatus {
  id: number;
  name: string;
  path: string | null;
  range_start: number;
  range_end: number;
  created_at: string;
  ports: PortStatus[];
}

// --- Multi-host (Phase 2) ---

/**
 * A remote Portsage backend the Mac UI knows about. Mirrors the Rust struct
 * `db::RemoteBackend` (snake_case fields preserved across the FFI boundary).
 */
export interface RemoteBackend {
  id: number;
  name: string;
  ssh_alias: string;
  remote_socket_path: string;
  local_socket_path: string;
  auto_forward_enabled: boolean;
  created_at: string;
}

/**
 * Owned form payload for create/update of a remote backend. Same shape as the
 * Rust `RemoteBackendForm`.
 */
export interface RemoteBackendForm {
  name: string;
  ssh_alias: string;
  remote_socket_path: string;
  local_socket_path: string;
  auto_forward_enabled: boolean;
}

/**
 * Which backend the UI is currently targeting. Serialized with an internal
 * tag so the Rust side can deserialize via serde.
 */
export type BackendTarget =
  | { kind: "local" }
  | { kind: "remote"; name: string };

/**
 * Lifecycle state of an SSH tunnel for a remote backend.
 */
export type TunnelState =
  | { state: "disconnected" }
  | { state: "connecting" }
  | { state: "connected" }
  | { state: "failed"; reason: string };

/**
 * Snapshot of a remote backend's tunnel. Emitted by the
 * `tunnel://state-changed` Tauri event.
 */
export interface TunnelStatus {
  backend_name: string;
  ssh_alias: string;
  remote_socket: string;
  local_socket: string;
  state: TunnelState;
}

// --- Forwards (Phase 3) ---

/**
 * Lifecycle state of a single SSH local-forward.
 */
export type ForwardState =
  | { state: "pending" }
  | { state: "active" }
  | { state: "failed"; reason: string }
  | { state: "cancelled" };

/**
 * Snapshot of one (backend, port) forward. Emitted by the
 * `forward://state-changed` Tauri event as a list (delta for the affected
 * backend).
 */
export interface ForwardStatus {
  backend_name: string;
  port: number;
  state: ForwardState;
}

/**
 * A port the user has explicitly blocked from auto-forwarding for a given
 * remote backend.
 */
export interface ForwardExclusion {
  id: number;
  backend_id: number;
  port: number;
  created_at: string;
}
