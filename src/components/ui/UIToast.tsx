import { useEffect } from "react";

export type ToastVariant = "error" | "success";

interface UIToastProps {
  message: string | null;
  variant?: ToastVariant;
  onDismiss: () => void;
  // Auto-dismiss after this many ms. 0 disables auto-dismiss.
  autoHideMs?: number;
}

const VARIANT_STYLES: Record<ToastVariant, { border: string; label: string; text: string }> = {
  error: {
    border: "border-accent-danger",
    label: "Error",
    text: "text-accent-danger",
  },
  success: {
    border: "border-accent-success",
    label: "Success",
    text: "text-accent-success",
  },
};

export function UIToast({
  message,
  variant = "error",
  onDismiss,
  autoHideMs = 6000,
}: UIToastProps) {
  useEffect(() => {
    if (!message || autoHideMs <= 0) return;
    const t = setTimeout(onDismiss, autoHideMs);
    return () => clearTimeout(t);
  }, [message, autoHideMs, onDismiss]);

  if (!message) return null;

  const styles = VARIANT_STYLES[variant];

  return (
    <div
      role={variant === "error" ? "alert" : "status"}
      className={`
        fixed bottom-[var(--spacing-4)] right-[var(--spacing-4)] z-50
        max-w-[420px]
        flex items-start gap-[var(--spacing-3)]
        px-[var(--spacing-4)] py-[var(--spacing-3)]
        bg-bg-elevated border ${styles.border}
        rounded-[var(--radius-md)]
        shadow-[0_4px_16px_rgba(0,0,0,0.4)]
        font-sans text-[13px] text-text-primary
      `}
    >
      <span className={`${styles.text} font-medium shrink-0`}>{styles.label}</span>
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
