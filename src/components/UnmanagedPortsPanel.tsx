import { UIText } from "@/components/ui/UIText";
import { UIDivider } from "@/components/ui/UIDivider";
import { UIStatus } from "@/components/ui/UIStatus";
import type { UnmanagedPort } from "@/lib/types";

interface UnmanagedPortsPanelProps {
  ports: UnmanagedPort[];
}

export function UnmanagedPortsPanel({ ports }: UnmanagedPortsPanelProps) {
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
            <div className="w-5" />
            <UIText variant="label" className="w-20">Port</UIText>
            <UIText variant="label" className="flex-1">Process</UIText>
            <UIText variant="label" className="w-16 text-right">PID</UIText>
          </div>
          {ports.map((port) => (
            <div
              key={port.port}
              className="flex items-center gap-[var(--spacing-2)] h-9 hover:bg-bg-elevated rounded-[var(--radius-sm)] px-[var(--spacing-1)]"
            >
              <div className="w-5 flex justify-center">
                <UIStatus active={true} />
              </div>
              <UIText variant="mono" className="w-20">
                {port.port}
              </UIText>
              <UIText variant="body" className="flex-1 truncate">
                {port.process}
              </UIText>
              <UIText variant="mono" className="w-16 text-right text-text-secondary text-[11px]!">
                {port.pid}
              </UIText>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
