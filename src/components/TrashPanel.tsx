import { useEffect, useState } from "react";
import { RotateCcw, Trash2 } from "lucide-react";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import { UIBadge } from "@/components/ui/UIBadge";
import { useConfirm } from "@/lib/dialog";
import { useToast } from "@/lib/toast";
import * as cmd from "@/lib/commands";
import type { TrashEntry } from "@/lib/types";

interface TrashPanelProps {
  /** Called after a restore so the caller can refetch its project list. */
  onRestored?: () => void;
}

/**
 * Deleted projects and ports, with a way to put them back. Deletions are
 * archived for 30 days; the backend purges older entries when it opens the
 * database, so nothing here needs a timer.
 */
export function TrashPanel({ onRestored }: TrashPanelProps) {
  const [entries, setEntries] = useState<TrashEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const confirm = useConfirm();
  const { showError, showSuccess } = useToast();

  const refresh = async () => {
    try {
      setEntries(await cmd.listTrash());
    } catch (e) {
      showError(`Could not read the trash: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void refresh();
    // Fetched once when the tab mounts; every mutation below refreshes it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleRestore = async (entry: TrashEntry) => {
    try {
      const outcome = await cmd.restoreTrash(entry.id);
      // A restore can succeed while leaving ports behind, and that must not
      // read as a clean success.
      if (outcome.skipped_ports.length > 0) {
        showError(
          `Restored ${outcome.project} without port${
            outcome.skipped_ports.length === 1 ? "" : "s"
          } ${outcome.skipped_ports.join(", ")}: registered to another project since`,
        );
      } else {
        showSuccess(`Restored ${outcome.project}`);
      }
      await refresh();
      onRestored?.();
    } catch (e) {
      showError(`${e}`);
    }
  };

  const handlePurge = async (entry: TrashEntry) => {
    const ok = await confirm({
      title: `Delete "${entry.label}" for good?`,
      message: "This entry will no longer be restorable.",
      kind: "warning",
      okLabel: "Delete",
      cancelLabel: "Cancel",
    });
    if (!ok) return;
    try {
      await cmd.purgeTrash(entry.id);
      await refresh();
    } catch (e) {
      showError(`${e}`);
    }
  };

  const handleEmpty = async () => {
    const ok = await confirm({
      title: "Empty the trash?",
      message: `${entries.length} entr${
        entries.length === 1 ? "y" : "ies"
      } will no longer be restorable.`,
      kind: "warning",
      okLabel: "Empty",
      cancelLabel: "Cancel",
    });
    if (!ok) return;
    try {
      await cmd.purgeTrash();
      await refresh();
    } catch (e) {
      showError(`${e}`);
    }
  };

  return (
    <div className="flex flex-col gap-[var(--spacing-3)]">
      <div className="flex items-start justify-between gap-[var(--spacing-3)]">
        <div className="flex flex-col gap-[var(--spacing-1)]">
          <UIText variant="section" as="h3">
            Trash
          </UIText>
          <UIText variant="body" className="text-text-secondary">
            Released projects and removed ports, restorable for 30 days.
          </UIText>
        </div>
        {entries.length > 0 && (
          <UIButton variant="danger" onClick={handleEmpty}>
            <Trash2 size={14} aria-hidden="true" />
            Empty
          </UIButton>
        )}
      </div>

      {loading ? (
        <UIText variant="body" className="text-text-muted">
          Loading...
        </UIText>
      ) : entries.length === 0 ? (
        <UIText variant="body" className="text-text-muted">
          Nothing deleted recently
        </UIText>
      ) : (
        <div className="flex flex-col">
          {entries.map((entry) => (
            <div
              key={entry.id}
              className="flex items-center gap-[var(--spacing-2)] h-9 hover:bg-bg-elevated rounded-[var(--radius-sm)] px-[var(--spacing-1)] group"
            >
              <UIBadge variant="inactive" className="shrink-0">
                {entry.kind}
              </UIBadge>
              <UIText variant="body" className="flex-1 min-w-0 truncate">
                {entry.label}
              </UIText>
              <UIText
                variant="mono"
                className="text-[11px]! text-text-secondary truncate max-w-[45%]"
              >
                {entry.detail}
              </UIText>
              <UIText variant="mono" className="text-[11px]! text-text-muted shrink-0">
                {entry.deleted_at.slice(0, 10)}
              </UIText>
              <div className="flex shrink-0">
                <UIButton
                  variant="ghost"
                  size="icon-sm"
                  className="opacity-0 group-hover:opacity-100"
                  title="Restore"
                  aria-label={`Restore ${entry.label}`}
                  onClick={() => handleRestore(entry)}
                >
                  <RotateCcw size={14} aria-hidden="true" />
                </UIButton>
                <UIButton
                  variant="danger"
                  size="icon-sm"
                  className="opacity-0 group-hover:opacity-100"
                  title="Delete for good"
                  aria-label={`Delete ${entry.label} for good`}
                  onClick={() => handlePurge(entry)}
                >
                  <Trash2 size={14} aria-hidden="true" />
                </UIButton>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
