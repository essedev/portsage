import { Trash2, Power, ArrowDownToLine } from "lucide-react";
import { UIStatus } from "@/components/ui/UIStatus";
import { UIText } from "@/components/ui/UIText";
import { UIButton } from "@/components/ui/UIButton";
import { UIPortLink } from "@/components/ui/UIPortLink";
import type { ForwardState, PortStatus } from "@/lib/types";

interface PortRowProps {
  port: PortStatus;
  onRemove?: (id: number) => void;
  onKill?: (port: PortStatus) => void;
  /**
   * Forward state for this port (Phase 3). Provided by `useForwards` when the
   * active backend is Remote. Undefined means "not applicable" (Local
   * backend) and the indicator is hidden. Renders an arrow icon next to the
   * port number reflecting Active/Failed/Pending/Cancelled.
   */
  forward?: ForwardState;
  /** Click handler for the forward indicator; toggles open/close. */
  onToggleForward?: (port: PortStatus) => void;
}

export function PortRow({
  port,
  onRemove,
  onKill,
  forward,
  onToggleForward,
}: PortRowProps) {
  // Inactive port: dim the service name and port number so the row reads as
  // "registered but not running". The status dot already encodes the state,
  // but tone-down on the row text makes scanning much faster.
  const inactiveTone = port.active ? "" : "text-text-muted!";

  return (
    <div className="flex items-center gap-[var(--spacing-2)] h-8 px-[var(--spacing-1)] rounded-[var(--radius-sm)] group hover:bg-bg-elevated transition-colors duration-150">
      <div className="w-5 flex justify-center shrink-0">
        <UIStatus active={port.active} />
      </div>
      <UIText variant="body" className={`flex-1 min-w-0 truncate ${inactiveTone}`}>
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
      <div className={`w-14 flex justify-end ${inactiveTone}`}>
        <UIPortLink port={port.port} />
      </div>
      <div className="w-6 flex justify-center shrink-0">
        {forward && (
          <ForwardIndicator
            forward={forward}
            port={port.port}
            onClick={onToggleForward ? () => onToggleForward(port) : undefined}
          />
        )}
      </div>
      {/* Action slots reserve their width even when empty, so rows with and
          without active ports stay vertically aligned. */}
      <div className="w-6 flex justify-center shrink-0">
        {onKill && port.active && (
          <UIButton
            variant="warning"
            size="icon-sm"
            className="opacity-0 group-hover:opacity-100"
            title="Stop process on this port"
            aria-label={`Stop process on port ${port.port}`}
            onClick={() => onKill(port)}
          >
            <Power size={14} aria-hidden="true" />
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
            aria-label={`Remove ${port.service} (port ${port.port}) from project`}
            onClick={() => onRemove(port.id)}
          >
            <Trash2 size={14} aria-hidden="true" />
          </UIButton>
        )}
      </div>
    </div>
  );
}

interface ForwardIndicatorProps {
  forward: ForwardState;
  port: number;
  onClick?: () => void;
}

/**
 * Small arrow icon next to the port number when the active backend is
 * Remote. Three rendered states:
 * - `active`: solid amber arrow with "Forwarded as localhost:<port>" tooltip.
 * - `failed`: dim red arrow with the failure reason on hover.
 * - `pending` / `cancelled` (no entry): subtle muted arrow inviting a click.
 * Clicking toggles via `onToggleForward`. When no handler is wired, the
 * indicator stays informational (no click).
 */
function ForwardIndicator({ forward, port, onClick }: ForwardIndicatorProps) {
  const { color, title, ariaLabel } = describeForward(forward, port);
  const baseClass =
    "inline-flex items-center justify-center w-5 h-5 rounded-[var(--radius-sm)] transition-colors duration-150";
  if (onClick) {
    return (
      <button
        type="button"
        onClick={onClick}
        title={title}
        aria-label={ariaLabel}
        className={`${baseClass} ${color} hover:bg-bg-elevated cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent-amber`}
      >
        <ArrowDownToLine size={12} aria-hidden="true" />
      </button>
    );
  }
  return (
    <span title={title} aria-label={ariaLabel} className={`${baseClass} ${color}`}>
      <ArrowDownToLine size={12} aria-hidden="true" />
    </span>
  );
}

function describeForward(
  forward: ForwardState,
  port: number,
): { color: string; title: string; ariaLabel: string } {
  switch (forward.state) {
    case "active":
      return {
        color: "text-accent-amber",
        title: `Forwarded as localhost:${port}`,
        ariaLabel: `Forward active for port ${port}`,
      };
    case "pending":
      return {
        color: "text-accent-amber animate-pulse",
        title: "Opening forward…",
        ariaLabel: `Opening forward for port ${port}`,
      };
    case "failed":
      return {
        color: "text-accent-danger",
        title: forward.reason,
        ariaLabel: `Forward failed for port ${port}: ${forward.reason}`,
      };
    case "cancelled":
      return {
        color: "text-text-muted",
        title: "Forward closed. Click to re-open.",
        ariaLabel: `Forward closed for port ${port}`,
      };
  }
}
