import { Power } from "lucide-react";
import { UIText } from "@/components/ui/UIText";
import { UIPageHeader } from "@/components/ui/UIPageHeader";
import { UITable } from "@/components/ui/UITable";
import { UIStatus } from "@/components/ui/UIStatus";
import { UIButton } from "@/components/ui/UIButton";
import { UIPortLink } from "@/components/ui/UIPortLink";
import { useConfirm } from "@/lib/dialog";
import { useToast } from "@/lib/toast";
import { describeKillOutcome } from "@/lib/killOutcome";
import type { UnmanagedPort, KillOutcome } from "@/lib/types";

interface UnmanagedPortsPanelProps {
  ports: UnmanagedPort[];
  onKill: (port: number) => Promise<KillOutcome | null>;
}

export function UnmanagedPortsPanel({ ports, onKill }: UnmanagedPortsPanelProps) {
  const confirm = useConfirm();
  const { showError, showSuccess } = useToast();

  const handleKill = async (p: UnmanagedPort) => {
    const ok = await confirm({
      title: `Stop port ${p.port}?`,
      message: `${p.process} (PID ${p.pid})\n\nSIGTERM will be sent. If the process does not exit within 2s, SIGKILL is sent.`,
      kind: "warning",
      okLabel: "Stop",
      cancelLabel: "Cancel",
    });
    if (!ok) return;
    const outcome = await onKill(p.port);
    if (!outcome) return;
    const report = describeKillOutcome(p.port, outcome);
    if (report.ok) showSuccess(report.message);
    else showError(report.message);
  };

  return (
    <div className="flex flex-col gap-[var(--spacing-4)] p-[var(--spacing-5)]">
      <UIPageHeader
        title="Unmanaged ports"
        subtitle="Active ports above 3000 not associated with any project"
      />

      <UITable
        columns={[
          {
            key: "status",
            width: "w-7",
            align: "center",
            cell: () => <UIStatus active={true} />,
          },
          {
            key: "process",
            header: "Process",
            cell: (p: UnmanagedPort) => (
              <UIText variant="body" className="truncate block">
                {p.process}
              </UIText>
            ),
          },
          {
            key: "pid",
            header: "PID",
            width: "w-20",
            align: "right",
            cell: (p: UnmanagedPort) => (
              <UIText variant="mono" className="text-text-secondary text-[11px]! tabular-nums">
                {p.pid}
              </UIText>
            ),
          },
          {
            key: "port",
            header: "Port",
            width: "w-16",
            align: "right",
            cell: (p: UnmanagedPort) => <UIPortLink port={p.port} />,
          },
          {
            key: "kill",
            width: "w-8",
            align: "center",
            cell: (p: UnmanagedPort) => (
              <UIButton
                variant="warning"
                size="icon-sm"
                className="opacity-0 group-hover:opacity-100"
                title="Stop process on this port"
                aria-label={`Stop process on port ${p.port}`}
                onClick={() => handleKill(p)}
              >
                <Power size={14} aria-hidden="true" />
              </UIButton>
            ),
          },
        ]}
        rows={ports}
        rowKey={(p) => p.port}
        empty="No unmanaged ports detected"
      />
    </div>
  );
}
