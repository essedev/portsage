import { Trash2 } from "lucide-react";
import { UIStatus } from "@/components/ui/UIStatus";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import type { PortStatus } from "@/lib/types";

interface PortRowProps {
  port: PortStatus;
  onRemove?: (id: number) => void;
}

export function PortRow({ port, onRemove }: PortRowProps) {
  return (
    <div className="flex items-center gap-[var(--spacing-2)] h-8 group">
      <UIStatus active={port.active} />
      <UIText variant="body" className="flex-1 truncate">
        {port.service}
      </UIText>
      {port.active && port.process && (
        <UIText variant="mono" className="text-text-muted text-[10px]! truncate max-w-32">
          {port.process}
        </UIText>
      )}
      <UIText variant="mono" className="tabular-nums">
        {port.port}
      </UIText>
      {onRemove && (
        <UIButton
          variant="danger"
          className="opacity-0 group-hover:opacity-100 p-1"
          onClick={() => onRemove(port.id)}
        >
          <Trash2 size={14} />
        </UIButton>
      )}
    </div>
  );
}
