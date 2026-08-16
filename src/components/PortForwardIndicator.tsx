import { ArrowDownToLine } from "lucide-react";
import type { ForwardState } from "@/lib/types";

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
export function ForwardIndicator({ forward, port, onClick }: ForwardIndicatorProps) {
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
