import { createContext, useCallback, useContext, useState, type ReactNode } from "react";
import { UIDialog, type DialogKind } from "@/components/ui/UIDialog";

export interface ConfirmOptions {
  title: string;
  message: string;
  kind?: DialogKind;
  okLabel?: string;
  cancelLabel?: string;
}

type Pending = ConfirmOptions & { resolve: (v: boolean) => void };

interface DialogContextValue {
  confirm: (opts: ConfirmOptions) => Promise<boolean>;
}

const DialogContext = createContext<DialogContextValue | null>(null);

export function DialogProvider({ children }: { children: ReactNode }) {
  const [pending, setPending] = useState<Pending | null>(null);

  // Imperative API: returns a Promise that resolves true on confirm,
  // false on cancel/escape/backdrop. If a previous dialog is still open
  // when a new one is requested, we resolve the old one as cancelled
  // and replace it - never leak a pending promise.
  const confirm = useCallback((opts: ConfirmOptions) => {
    return new Promise<boolean>((resolve) => {
      setPending((prev) => {
        prev?.resolve(false);
        return { ...opts, resolve };
      });
    });
  }, []);

  const close = (result: boolean) => {
    setPending((prev) => {
      prev?.resolve(result);
      return null;
    });
  };

  return (
    <DialogContext.Provider value={{ confirm }}>
      {children}
      <UIDialog
        open={pending !== null}
        title={pending?.title ?? ""}
        message={pending?.message ?? ""}
        kind={pending?.kind ?? "info"}
        okLabel={pending?.okLabel ?? "OK"}
        cancelLabel={pending?.cancelLabel ?? "Cancel"}
        onConfirm={() => close(true)}
        onCancel={() => close(false)}
      />
    </DialogContext.Provider>
  );
}

// No-op fallback for windows without a provider (e.g. the popover).
// Returns false so any guarded mutation simply doesn't run.
const NOOP_DIALOG: DialogContextValue = {
  confirm: async () => false,
};

export function useConfirm(): DialogContextValue["confirm"] {
  return (useContext(DialogContext) ?? NOOP_DIALOG).confirm;
}
