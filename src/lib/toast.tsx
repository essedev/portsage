import { createContext, useCallback, useContext, useState, type ReactNode } from "react";
import { UIToast, type ToastVariant } from "@/components/ui/UIToast";

interface ToastState {
  message: string;
  variant: ToastVariant;
}

interface ToastContextValue {
  showError: (message: string) => void;
  showSuccess: (message: string) => void;
  clear: () => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toast, setToast] = useState<ToastState | null>(null);

  const showError = useCallback((message: string) => {
    setToast({ message, variant: "error" });
  }, []);

  const showSuccess = useCallback((message: string) => {
    setToast({ message, variant: "success" });
  }, []);

  const clear = useCallback(() => setToast(null), []);

  return (
    <ToastContext.Provider value={{ showError, showSuccess, clear }}>
      {children}
      <UIToast
        message={toast?.message ?? null}
        variant={toast?.variant ?? "error"}
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
