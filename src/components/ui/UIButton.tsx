import { type ReactNode, type ButtonHTMLAttributes } from "react";

type ButtonVariant = "primary" | "ghost" | "danger";

interface UIButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  children: ReactNode;
}

const variantClasses: Record<ButtonVariant, string> = {
  primary:
    "bg-accent-amber text-bg-deep hover:bg-accent-amber/90 border-transparent",
  ghost:
    "bg-transparent text-text-secondary hover:text-text-primary hover:bg-bg-elevated border-transparent",
  danger:
    "bg-transparent text-accent-danger hover:bg-accent-danger-soft border-transparent",
};

export function UIButton({
  variant = "ghost",
  children,
  className = "",
  ...props
}: UIButtonProps) {
  return (
    <button
      className={`
        inline-flex items-center justify-center gap-[var(--spacing-1)]
        px-[var(--spacing-3)] py-[var(--spacing-1)]
        rounded-[var(--radius-sm)] border
        font-sans text-[13px] font-medium
        transition-colors duration-150
        cursor-pointer
        disabled:opacity-40 disabled:pointer-events-none
        focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-amber
        ${variantClasses[variant]} ${className}
      `}
      {...props}
    >
      {children}
    </button>
  );
}
