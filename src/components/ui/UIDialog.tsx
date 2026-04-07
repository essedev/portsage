import { useEffect, useRef } from "react";
import { UIButton } from "@/components/ui/UIButton";
import { UIText } from "@/components/ui/UIText";

// React 19 removed `forwardRef` for function components but `UIButton`
// doesn't currently destructure `ref`, so we use `autoFocus` on the
// confirm button instead. The dialog unmounts/remounts on every open
// (controlled by the `open` prop returning early), so `autoFocus` fires
// every time, not just the first.

export type DialogKind = "warning" | "info";

interface UIDialogProps {
  open: boolean;
  title: string;
  message: string;
  kind?: DialogKind;
  okLabel?: string;
  cancelLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

export function UIDialog({
  open,
  title,
  message,
  kind = "info",
  okLabel = "OK",
  cancelLabel = "Cancel",
  onConfirm,
  onCancel,
}: UIDialogProps) {
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);

  // Focus management: when the dialog opens, remember the element that had
  // focus so we can restore it on close. The confirm button itself grabs
  // focus via `autoFocus` below.
  useEffect(() => {
    if (!open) return;
    previouslyFocusedRef.current = document.activeElement as HTMLElement | null;
    return () => {
      previouslyFocusedRef.current?.focus?.();
    };
  }, [open]);

  // Esc cancels, Enter confirms. Bound globally only while the dialog is
  // open so we don't intercept anything else. Tab is left to the browser
  // since we have only two focusable elements - it cycles between them
  // naturally without an explicit focus trap.
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      } else if (e.key === "Enter") {
        e.preventDefault();
        onConfirm();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onCancel, onConfirm]);

  if (!open) return null;

  const confirmVariant = kind === "warning" ? "danger" : "primary";

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="ui-dialog-title"
      aria-describedby="ui-dialog-message"
      className="fixed inset-0 z-50 flex items-center justify-center"
    >
      {/* Backdrop - clicking it cancels, mirrors the native sheet behavior. */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-[2px]"
        onClick={onCancel}
      />

      <div
        className="
          relative z-10
          flex flex-col gap-[var(--spacing-4)]
          w-[min(440px,calc(100vw-32px))]
          bg-bg-elevated border border-border-subtle
          rounded-[var(--radius-lg)]
          p-[var(--spacing-5)]
          shadow-[0_12px_40px_rgba(0,0,0,0.5)]
        "
        // Stop clicks inside the panel from bubbling up to the backdrop.
        onClick={(e) => e.stopPropagation()}
      >
        <h2 id="ui-dialog-title">
          <UIText variant="section" as="span">
            {title}
          </UIText>
        </h2>

        <div id="ui-dialog-message">
          <UIText variant="body" className="text-text-secondary leading-relaxed">
            {message}
          </UIText>
        </div>

        <div className="flex items-center justify-end gap-[var(--spacing-2)] pt-[var(--spacing-2)]">
          <UIButton variant="ghost" onClick={onCancel}>
            {cancelLabel}
          </UIButton>
          <UIButton
            variant={confirmVariant}
            onClick={onConfirm}
            autoFocus
          >
            {okLabel}
          </UIButton>
        </div>
      </div>
    </div>
  );
}
