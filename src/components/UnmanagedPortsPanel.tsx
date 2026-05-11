import { Power } from "lucide-react";
import { UIText } from "@/components/ui/UIText";
import { UIDivider } from "@/components/ui/UIDivider";
import { UIStatus } from "@/components/ui/UIStatus";
import { UIButton } from "@/components/ui/UIButton";
import { UIPortLink } from "@/components/ui/UIPortLink";
import { useConfirm } from "@/lib/dialog";
import { useToast } from "@/lib/toast";
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
    switch (outcome) {
      case "terminated":
        showSuccess(`Port ${p.port} stopped`);
        break;
      case "killed":
        showSuccess(`Port ${p.port} force-killed (SIGKILL)`);
        break;
      case "not_active":
        showSuccess(`Port ${p.port} was already free`);
        break;
      case "permission_denied":
        showError(`Cannot stop port ${p.port}: permission denied (different user?)`);
        break;
    }
  };

  return (
    <div className="flex flex-col gap-[var(--spacing-4)] p-[var(--spacing-5)]">
      <div className="flex flex-col gap-[var(--spacing-1)]">
        <UIText variant="title" as="h2">
          Unmanaged ports
        </UIText>
        <UIText variant="body" className="text-text-secondary">
          Active ports above 3000 not associated with any project
        </UIText>
      </div>

      <UIDivider />

      {ports.length === 0 ? (
        <UIText variant="body" className="text-text-muted">
          No unmanaged ports detected
        </UIText>
      ) : (
        <div className="flex flex-col">
          <div className="flex items-center gap-[var(--spacing-2)] pb-[var(--spacing-2)] mb-[var(--spacing-1)] border-b border-border-subtle">
            <div className="w-5 shrink-0" />
            <UIText variant="label" className="flex-1 min-w-0">Process</UIText>
            <UIText variant="label" className="w-16 text-right">PID</UIText>
            <UIText variant="label" className="w-14 text-right">Port</UIText>
            <div className="w-6 shrink-0" />
          </div>
          {ports.map((port) => (
            <div
              key={port.port}
              className="flex items-center gap-[var(--spacing-2)] h-9 hover:bg-bg-elevated rounded-[var(--radius-sm)] px-[var(--spacing-1)] group"
            >
              <div className="w-5 flex justify-center shrink-0">
                <UIStatus active={true} />
              </div>
              <UIText variant="body" className="flex-1 min-w-0 truncate">
                {port.process}
              </UIText>
              <UIText variant="mono" className="w-16 text-right text-text-secondary text-[11px]! tabular-nums">
                {port.pid}
              </UIText>
              <div className="w-14 flex justify-end">
                <UIPortLink port={port.port} />
              </div>
              <div className="w-6 flex justify-center shrink-0">
                <UIButton
                  variant="ghost"
                  size="icon-sm"
                  className="opacity-0 group-hover:opacity-100"
                  title="Stop process on this port"
                  onClick={() => handleKill(port)}
                >
                  <Power size={14} />
                </UIButton>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
