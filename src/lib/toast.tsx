import { createContext, useCallback, useContext, useState, type ReactNode } from "react";
import { UIToast, type ToastVariant } from "@/components/ui/UIToast";

/** An inline button in the toast, used for "Undo" right after a deletion. */
export interface ToastAction {
  label: string;
  onClick: () => void;
}

interface ToastState {
  message: string;
  variant: ToastVariant;
  action?: ToastAction;
}

interface ToastContextValue {
  showError: (message: string) => void;
  showSuccess: (message: string, action?: ToastAction) => void;
  clear: () => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toast, setToast] = useState<ToastState | null>(null);

  const showError = useCallback((message: string) => {
    setToast({ message, variant: "error" });
  }, []);

  const showSuccess = useCallback((message: string, action?: ToastAction) => {
    setToast({ message, variant: "success", action });
  }, []);

  const clear = useCallback(() => setToast(null), []);

  return (
    <ToastContext.Provider value={{ showError, showSuccess, clear }}>
      {children}
      <UIToast
        message={toast?.message ?? null}
        variant={toast?.variant ?? "error"}
        action={
          toast?.action && {
            label: toast.action.label,
            // Dismiss on click: leaving an "Undo" button up after it has
            // been used invites a second, confusing press.
            onClick: () => {
              toast.action?.onClick();
              clear();
            },
          }
        }
        onDismiss={clear}
      />
    </ToastContext.Provider>
  );
}

// No-op fallback used when no provider is mounted (e.g. inside the popover
// window, which shares hooks with the main window but is read-only and
// shouldn't show toasts in its 350x480 viewport).
const NOOP_TOAST: ToastContextValue = {
  showError: () => {},
  showSuccess: () => {},
  clear: () => {},
};

export function useToast(): ToastContextValue {
  return useContext(ToastContext) ?? NOOP_TOAST;
}
