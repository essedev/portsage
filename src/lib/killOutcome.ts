import type { KillEntry } from "@/lib/commands";
import type { KillOutcome } from "@/lib/types";

export interface KillReport {
  ok: boolean;
  message: string;
}

/**
 * One entry per KillOutcome, keyed by a Record rather than a switch so that
 * adding a variant to the union breaks the typecheck instead of silently
 * falling through to no message at all.
 *
 * `label` is the plural-friendly wording used when several ports share the
 * same outcome; `single` is the message for one port.
 */
const OUTCOMES: Record<KillOutcome, { ok: boolean; single: (port: number) => string; label: string }> = {
  terminated: {
    ok: true,
    single: (port) => `Port ${port} stopped`,
    label: "stopped",
  },
  killed: {
    ok: true,
    single: (port) => `Port ${port} force-killed (SIGKILL)`,
    label: "force-killed",
  },
  not_active: {
    ok: true,
    single: (port) => `Port ${port} was already free`,
    label: "already free",
  },
  permission_denied: {
    ok: false,
    single: (port) => `Cannot stop port ${port}: permission denied (different user?)`,
    label: "permission denied",
  },
  docker_stopped: {
    ok: true,
    single: (port) => `Port ${port} container stopped (docker)`,
    label: "containers stopped",
  },
  docker_cli_missing: {
    ok: false,
    single: (port) =>
      `Cannot stop port ${port}: docker CLI not found. Set PORTSAGE_DOCKER_BIN to its path.`,
    label: "docker CLI not found",
  },
  docker_daemon_down: {
    ok: false,
    single: (port) => `Cannot stop port ${port}: the Docker daemon is not running`,
    label: "Docker daemon not running",
  },
  docker_no_container: {
    ok: false,
    single: (port) => `Cannot stop port ${port}: no running container publishes it`,
    label: "no matching container",
  },
  docker_error: {
    ok: false,
    single: (port) => `Cannot stop port ${port}: docker stop failed`,
    label: "docker stop failed",
  },
};

export function describeKillOutcome(port: number, outcome: KillOutcome): KillReport {
  const entry = OUTCOMES[outcome];
  return { ok: entry.ok, message: entry.single(port) };
}

/**
 * Collapse a kill-project result into the messages to show: one line per
 * distinct failure (listing its ports) plus a single success summary. Errors
 * are never swallowed by a successful sibling, which is why they are returned
 * as a list rather than a first-match-wins string.
 */
export function summarizeKillEntries(entries: KillEntry[]): {
  errors: string[];
  success: string | null;
} {
  if (entries.length === 0) return { errors: [], success: "No active ports to stop" };

  const byOutcome = new Map<KillOutcome, number[]>();
  for (const entry of entries) {
    const ports = byOutcome.get(entry.outcome) ?? [];
    ports.push(entry.port);
    byOutcome.set(entry.outcome, ports);
  }

  const errors: string[] = [];
  const successParts: string[] = [];
  for (const [outcome, ports] of byOutcome) {
    const { ok, label } = OUTCOMES[outcome];
    if (ok) {
      successParts.push(`${ports.length} ${label}`);
    } else {
      errors.push(`${label}: port${ports.length === 1 ? "" : "s"} ${ports.join(", ")}`);
    }
  }
  return { errors, success: successParts.length > 0 ? successParts.join(", ") : null };
}
