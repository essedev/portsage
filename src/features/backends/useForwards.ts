import { useCallback, useEffect, useMemo, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as cmd from "@/lib/commands";
import { humanizeError } from "@/lib/errors";
import { useToast } from "@/lib/toast";
import type {
  BackendTarget,
  ForwardState,
  ForwardStatus,
} from "@/lib/types";

/**
 * Subscribe to SSH-forward state for a single remote backend.
 *
 * Mirrors `useBackends` shape but lives separately because the consumer
 * scope is narrower: only the project-detail / port-row UIs care about
 * forwards, and only when the active backend is Remote. The hook is a
 * no-op when `target` is Local or null.
 */
export interface ForwardsApi {
  /** Forward state keyed by port number. Empty when target is Local. */
  byPort: Record<number, ForwardState>;
  loading: boolean;
  /** Force-open a forward. */
  enable: (port: number) => Promise<boolean>;
  /** Force-close a forward. */
  disable: (port: number) => Promise<boolean>;
  /** Reconcile the full set against registered ports. */
  sync: () => Promise<void>;
}

export function useForwards(target: BackendTarget | null): ForwardsApi {
  const [byPort, setByPort] = useState<Record<number, ForwardState>>({});
  const [loading, setLoading] = useState(false);
  const { showError } = useToast();

  // Resolve the active backend name (or null when Local / unset).
  const backendName = target?.kind === "remote" ? target.name : null;

  const ingest = useCallback((statuses: ForwardStatus[]) => {
    setByPort((prev) => {
      const next = { ...prev };
      for (const s of statuses) {
        next[s.port] = s.state;
      }
      return next;
    });
  }, []);

  const refresh = useCallback(async () => {
    if (!backendName) {
      setByPort({});
      return;
    }
    setLoading(true);
    try {
      const statuses = await cmd.listForwardStatuses(backendName);
      const initial: Record<number, ForwardState> = {};
      for (const s of statuses) initial[s.port] = s.state;
      setByPort(initial);
    } catch (err) {
      showError(humanizeError(err));
    } finally {
      setLoading(false);
    }
  }, [backendName, showError]);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    void (async () => {
      await refresh();
      unlisten = await listen<ForwardStatus[]>(cmd.FORWARD_EVENT, (event) => {
        // Filter to events for the currently-active backend; others (e.g.
        // a background sync on a different backend) should not affect this
        // hook's state.
        const relevant = event.payload.filter(
          (s) => s.backend_name === backendName,
        );
        if (relevant.length > 0) ingest(relevant);
      });
    })();
    return () => {
      unlisten?.();
    };
  }, [backendName, refresh, ingest]);

  const enable = useCallback(
    async (port: number): Promise<boolean> => {
      if (!backendName) return false;
      try {
        const status = await cmd.enableForward(backendName, port);
        ingest([status]);
        return true;
      } catch (err) {
        showError(humanizeError(err));
        // The Rust side records the failed state too; refresh so the dot
        // reflects "failed" rather than "no forward at all".
        void refresh();
        return false;
      }
    },
    [backendName, ingest, refresh, showError],
  );

  const disable = useCallback(
    async (port: number): Promise<boolean> => {
      if (!backendName) return false;
      try {
        const status = await cmd.disableForward(backendName, port);
        ingest([status]);
        return true;
      } catch (err) {
        showError(humanizeError(err));
        return false;
      }
    },
    [backendName, ingest, showError],
  );

  const sync = useCallback(async () => {
    if (!backendName) return;
    try {
      const statuses = await cmd.syncForwards(backendName);
      const next: Record<number, ForwardState> = {};
      for (const s of statuses) next[s.port] = s.state;
      setByPort(next);
    } catch (err) {
      showError(humanizeError(err));
    }
  }, [backendName, showError]);

  return useMemo(
    () => ({ byPort, loading, enable, disable, sync }),
    [byPort, loading, enable, disable, sync],
  );
}
