import { useEffect, useMemo, useState } from "react";
import { Archive, FolderX, Clock } from "lucide-react";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import { UIDivider } from "@/components/ui/UIDivider";
import { UIPageHeader } from "@/components/ui/UIPageHeader";
import { UITable } from "@/components/ui/UITable";
import { useToast } from "@/lib/toast";
import { humanizeError } from "@/lib/errors";
import * as cmd from "@/lib/commands";
import type { StaleProject } from "@/lib/types";

interface PrunePanelProps {
  /** Called after projects are archived so the caller can refetch. */
  onArchived?: () => void;
}

const THRESHOLDS = [30, 60, 90, 180];

/**
 * Projects that look abandoned, and a way to shelve them. Nothing is deleted
 * here: archiving keeps the range, the ports and the name, and the project
 * comes back on its own the moment one of its ports is listening again.
 */
export function PrunePanel({ onArchived }: PrunePanelProps) {
  const [days, setDays] = useState(90);
  const [candidates, setCandidates] = useState<StaleProject[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState(false);
  const { showError, showSuccess } = useToast();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    cmd
      .listStale(days)
      .then((list) => {
        if (cancelled) return;
        setCandidates(list);
        // Everything pre-selected: the backend already excluded anything
        // with a listening port, so the default is "all of it".
        setSelected(new Set(list.map((c) => c.name)));
      })
      .catch((err) => !cancelled && showError(humanizeError(err)))
      .finally(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [days, showError]);

  const missingCount = useMemo(
    () => candidates.filter((c) => c.reason === "path_missing").length,
    [candidates],
  );

  const toggle = (name: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  const handleArchive = async () => {
    const names = candidates.map((c) => c.name).filter((n) => selected.has(n));
    if (names.length === 0) return;
    setWorking(true);
    const failures: string[] = [];
    for (const name of names) {
      try {
        await cmd.setProjectArchived(name, true);
      } catch (err) {
        failures.push(`${name}: ${humanizeError(err)}`);
      }
    }
    setWorking(false);
    const archived = names.length - failures.length;
    if (archived > 0) {
      showSuccess(`Archived ${archived} project${archived === 1 ? "" : "s"}`);
    }
    failures.forEach((f) => showError(f));
    setCandidates((prev) => prev.filter((c) => !selected.has(c.name) || failures.some((f) => f.startsWith(`${c.name}:`))));
    onArchived?.();
  };

  return (
    <div className="flex flex-col gap-[var(--spacing-4)] p-[var(--spacing-5)]">
      <UIPageHeader
        title="Prune"
        subtitle="Projects whose folder is gone, or untouched for a while. Archiving keeps the range and the ports; a project comes back by itself when one of its ports starts listening."
        divider={false}
      />

      <div className="flex items-center gap-[var(--spacing-2)]">
        <UIText variant="label">Idle for at least</UIText>
        {THRESHOLDS.map((t) => (
          <UIButton
            key={t}
            variant={t === days ? "primary" : "ghost"}
            className="text-[12px]!"
            onClick={() => setDays(t)}
          >
            {t}d
          </UIButton>
        ))}
      </div>

      <UIDivider />

      {loading ? (
        <UIText variant="body" className="text-text-muted">
          Checking...
        </UIText>
      ) : candidates.length === 0 ? (
        <UIText variant="body" className="text-text-muted">
          Nothing idle for {days}+ days. Projects with a port listening right now are never listed.
        </UIText>
      ) : (
        <>
          <UITable
            columns={[
              {
                key: "pick",
                width: "w-10",
                align: "center",
                cell: (c: StaleProject) => (
                  <input
                    type="checkbox"
                    checked={selected.has(c.name)}
                    onChange={() => toggle(c.name)}
                    // The row itself toggles too; without this the click
                    // would bubble up and undo what the box just did.
                    onClick={(e) => e.stopPropagation()}
                    aria-label={`Select ${c.name}`}
                    className="accent-[var(--color-accent-amber)] cursor-pointer"
                  />
                ),
              },
              {
                key: "name",
                header: "Project",
                cell: (c: StaleProject) => (
                  <UIText variant="body" className="truncate block">
                    {c.name}
                  </UIText>
                ),
              },
              {
                key: "range",
                header: "Range",
                width: "w-28",
                align: "right",
                cell: (c: StaleProject) => (
                  <UIText variant="mono" className="text-[11px]! text-text-secondary tabular-nums">
                    {c.range_start}-{c.range_end}
                  </UIText>
                ),
              },
              {
                key: "ports",
                header: "Ports",
                width: "w-16",
                align: "right",
                cell: (c: StaleProject) => (
                  <UIText variant="mono" className="text-[11px]! text-text-muted tabular-nums">
                    {c.registered_ports}
                  </UIText>
                ),
              },
              {
                key: "why",
                header: "Why",
                width: "w-40",
                cell: (c: StaleProject) =>
                  c.reason === "path_missing" ? (
                    <span className="flex items-center gap-[var(--spacing-1)]">
                      <FolderX size={12} className="text-accent-danger shrink-0" />
                      <UIText variant="mono" className="text-[11px]! text-text-muted truncate">
                        folder gone
                      </UIText>
                    </span>
                  ) : (
                    <span className="flex items-center gap-[var(--spacing-1)]">
                      <Clock size={12} className="text-text-muted shrink-0" />
                      <UIText variant="mono" className="text-[11px]! text-text-muted">
                        {c.inactive_days}d idle
                      </UIText>
                    </span>
                  ),
              },
            ]}
            rows={candidates}
            rowKey={(c) => c.name}
            // Clicking anywhere on the row toggles it: the checkbox alone is
            // a small target for a list you tick through.
            onRowClick={(c) => toggle(c.name)}
          />

          {missingCount > 0 && (
            <UIText variant="body" className="text-text-muted text-[12px]!">
              {missingCount === 1 ? "One project has" : `${missingCount} projects have`} a folder
              that no longer exists. If you moved it, fix the path from the project page instead of
              archiving.
            </UIText>
          )}

          <div className="flex items-center gap-[var(--spacing-2)]">
            <UIButton variant="primary" onClick={handleArchive} disabled={working || selected.size === 0}>
              <Archive size={14} aria-hidden="true" />
              Archive {selected.size} selected
            </UIButton>
          </div>
        </>
      )}
    </div>
  );
}
