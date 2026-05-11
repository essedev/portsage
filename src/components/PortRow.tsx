import { Trash2, Power } from "lucide-react";
import { UIStatus } from "@/components/ui/UIStatus";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import { UIPortLink } from "@/components/ui/UIPortLink";
import type { PortStatus } from "@/lib/types";

interface PortRowProps {
  port: PortStatus;
  onRemove?: (id: number) => void;
  onKill?: (port: PortStatus) => void;
}

export function PortRow({ port, onRemove, onKill }: PortRowProps) {
  return (
    <div className="flex items-center gap-[var(--spacing-2)] h-8 px-[var(--spacing-1)] rounded-[var(--radius-sm)] group hover:bg-bg-elevated transition-colors duration-150">
      <div className="w-5 flex justify-center shrink-0">
        <UIStatus active={port.active} />
      </div>
      <UIText variant="body" className="flex-1 min-w-0 truncate">
        {port.service}
      </UIText>
      <UIText variant="mono" className="w-32 truncate text-text-muted text-[11px]!">
        {port.active && port.process ? port.process : ""}
      </UIText>
      <UIText
        variant="mono"
        className="w-16 text-right text-text-secondary text-[11px]! tabular-nums"
      >
        {port.pid ?? ""}
      </UIText>
      <div className="w-14 flex justify-end">
        <UIPortLink port={port.port} />
      </div>
      {/* Action slots reserve their width even when empty, so rows with and
          without active ports stay vertically aligned. */}
      <div className="w-6 flex justify-center shrink-0">
        {onKill && port.active && (
          <UIButton
            variant="ghost"
            size="icon-sm"
            className="opacity-0 group-hover:opacity-100"
            title="Stop process on this port"
            onClick={() => onKill(port)}
          >
            <Power size={14} />
          </UIButton>
        )}
      </div>
      <div className="w-6 flex justify-center shrink-0">
        {onRemove && (
          <UIButton
            variant="danger"
            size="icon-sm"
            className="opacity-0 group-hover:opacity-100"
            title="Remove port from project"
            onClick={() => onRemove(port.id)}
          >
            <Trash2 size={14} />
          </UIButton>
        )}
      </div>
    </div>
  );
}
