import { GrimText } from "@/components/ui/GrimText";
import { GrimDivider } from "@/components/ui/GrimDivider";
import { GrimStatus } from "@/components/ui/GrimStatus";
import type { UnmanagedPort } from "@/lib/types";

interface UnmanagedPortsPanelProps {
  ports: UnmanagedPort[];
}

export function UnmanagedPortsPanel({ ports }: UnmanagedPortsPanelProps) {
  return (
    <div className="flex flex-col gap-[var(--spacing-4)] p-[var(--spacing-5)]">
      <div className="flex flex-col gap-[var(--spacing-1)]">
        <GrimText variant="title" as="h2">
          Unmanaged ports
        </GrimText>
        <GrimText variant="body" className="text-text-secondary">
          Active ports above 3000 not associated with any project
        </GrimText>
      </div>

      <GrimDivider />

      {ports.length === 0 ? (
        <GrimText variant="body" className="text-text-muted">
          No unmanaged ports detected
        </GrimText>
      ) : (
        <div className="flex flex-col">
          <div className="flex items-center gap-[var(--spacing-2)] pb-[var(--spacing-2)] mb-[var(--spacing-1)] border-b border-border-subtle">
            <div className="w-5" />
            <GrimText variant="label" className="w-20">Port</GrimText>
            <GrimText variant="label" className="flex-1">Process</GrimText>
            <GrimText variant="label" className="w-16 text-right">PID</GrimText>
          </div>
          {ports.map((port) => (
            <div
              key={port.port}
              className="flex items-center gap-[var(--spacing-2)] h-9 hover:bg-bg-elevated rounded-[var(--radius-sm)] px-[var(--spacing-1)]"
            >
              <div className="w-5 flex justify-center">
                <GrimStatus active={true} />
              </div>
              <GrimText variant="mono" className="w-20">
                {port.port}
              </GrimText>
              <GrimText variant="body" className="flex-1 truncate">
                {port.process}
              </GrimText>
              <GrimText variant="mono" className="w-16 text-right text-text-secondary text-[11px]!">
                {port.pid}
              </GrimText>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
