import { useEffect, useState } from "react";
import { RotateCcw, Trash2 } from "lucide-react";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import { UIBadge } from "@/components/ui/UIBadge";
import { UIPageHeader } from "@/components/ui/UIPageHeader";
import { UITable } from "@/components/ui/UITable";
import { useConfirm } from "@/lib/dialog";
import { useToast } from "@/lib/toast";
import * as cmd from "@/lib/commands";
import type { TrashEntry } from "@/lib/types";

interface TrashPanelProps {
  /** Called after a restore so the caller can refetch its project list. */
  onRestored?: () => void;
  /** Called after any change to the entry count (restore, purge). */
  onChanged?: () => void;
}

/**
 * Deleted projects and ports, with a way to put them back. Deletions are
 * archived for 30 days; the backend purges older entries when it opens the
 * database, so nothing here needs a timer.
 */
export function TrashPanel({ onRestored, onChanged }: TrashPanelProps) {
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
      onChanged?.();
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
      onChanged?.();
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
      onChanged?.();
    } catch (e) {
      showError(`${e}`);
    }
  };

  return (
    <div className="flex flex-col gap-[var(--spacing-4)] p-[var(--spacing-5)]">
      <UIPageHeader
        title="Trash"
        subtitle="Released projects and removed ports, restorable for 30 days."
        actions={
          entries.length > 0 && (
            <UIButton variant="danger" onClick={handleEmpty}>
              <Trash2 size={14} aria-hidden="true" />
              Empty
            </UIButton>
          )
        }
      />

      {loading ? (
        <UIText variant="body" className="text-text-muted">
          Loading...
        </UIText>
      ) : (
        <UITable
          columns={[
            {
              key: "kind",
              width: "w-20",
              cell: (e: TrashEntry) => <UIBadge variant="inactive">{e.kind}</UIBadge>,
            },
            {
              key: "label",
              header: "Name",
              cell: (e: TrashEntry) => (
                <UIText variant="body" className="truncate block">
                  {e.label}
                </UIText>
              ),
            },
            {
              key: "detail",
              header: "What",
              width: "w-48",
              cell: (e: TrashEntry) => (
                <UIText variant="mono" className="truncate block text-[11px]! text-text-secondary">
                  {e.detail}
                </UIText>
              ),
            },
            {
              key: "deleted",
              header: "Deleted",
              width: "w-24",
              align: "right",
              cell: (e: TrashEntry) => (
                <UIText variant="mono" className="text-[11px]! text-text-muted tabular-nums">
                  {e.deleted_at.slice(0, 10)}
                </UIText>
              ),
            },
            {
              key: "restore",
              width: "w-8",
              align: "center",
              cell: (e: TrashEntry) => (
                <UIButton
                  variant="ghost"
                  size="icon-sm"
                  className="opacity-0 group-hover:opacity-100"
                  title="Restore"
                  aria-label={`Restore ${e.label}`}
                  onClick={() => handleRestore(e)}
                >
                  <RotateCcw size={14} aria-hidden="true" />
                </UIButton>
              ),
            },
            {
              key: "purge",
              width: "w-8",
              align: "center",
              cell: (e: TrashEntry) => (
                <UIButton
                  variant="danger"
                  size="icon-sm"
                  className="opacity-0 group-hover:opacity-100"
                  title="Delete for good"
                  aria-label={`Delete ${e.label} for good`}
                  onClick={() => handlePurge(e)}
                >
                  <Trash2 size={14} aria-hidden="true" />
                </UIButton>
              ),
            },
          ]}
          rows={entries}
          rowKey={(e) => e.id}
          empty="Nothing deleted recently"
        />
      )}
    </div>
  );
}
