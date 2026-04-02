import { Trash2 } from "lucide-react";
import { GrimStatus } from "@/components/ui/GrimStatus";
import { GrimText } from "@/components/ui/GrimText";
import { GrimButton } from "@/components/ui/GrimButton";
import type { PortStatus } from "@/lib/types";

interface PortRowProps {
  port: PortStatus;
  onRemove?: (id: number) => void;
}

export function PortRow({ port, onRemove }: PortRowProps) {
  return (
    <div className="flex items-center gap-[var(--spacing-2)] h-8 group">
      <GrimStatus active={port.active} />
      <GrimText variant="body" className="flex-1 truncate">
        {port.service}
      </GrimText>
      {port.active && port.process && (
        <GrimText variant="mono" className="text-text-muted text-[10px]! truncate max-w-32">
          {port.process}
        </GrimText>
      )}
      <GrimText variant="mono" className="tabular-nums">
        {port.port}
      </GrimText>
      {onRemove && (
        <GrimButton
          variant="danger"
          className="opacity-0 group-hover:opacity-100 p-1"
          onClick={() => onRemove(port.id)}
        >
          <Trash2 size={14} />
        </GrimButton>
      )}
    </div>
  );
}
