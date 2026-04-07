import { useEffect } from "react";

interface GrimToastProps {
  message: string | null;
  onDismiss: () => void;
  // Auto-dismiss after this many ms. 0 disables auto-dismiss.
  autoHideMs?: number;
}

export function GrimToast({ message, onDismiss, autoHideMs = 6000 }: GrimToastProps) {
  useEffect(() => {
    if (!message || autoHideMs <= 0) return;
    const t = setTimeout(onDismiss, autoHideMs);
    return () => clearTimeout(t);
  }, [message, autoHideMs, onDismiss]);

  if (!message) return null;

  return (
    <div
      role="alert"
      className="
        fixed bottom-[var(--spacing-4)] right-[var(--spacing-4)] z-50
        max-w-[420px]
        flex items-start gap-[var(--spacing-3)]
        px-[var(--spacing-4)] py-[var(--spacing-3)]
        bg-bg-elevated border border-accent-danger
        rounded-[var(--radius-md)]
        shadow-[0_4px_16px_rgba(0,0,0,0.4)]
        font-sans text-[13px] text-text-primary
      "
    >
      <span className="text-accent-danger font-medium shrink-0">Error</span>
      <span className="flex-1 break-words">{message}</span>
      <button
        onClick={onDismiss}
        aria-label="Dismiss"
        className="
          shrink-0 text-text-muted hover:text-text-primary
          cursor-pointer transition-colors
        "
      >
        ×
      </button>
    </div>
  );
}
