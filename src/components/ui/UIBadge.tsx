import { type ReactNode } from "react";

type BadgeVariant = "active" | "inactive" | "danger";

interface UIBadgeProps {
  variant?: BadgeVariant;
  children: ReactNode;
  className?: string;
}

const variantClasses: Record<BadgeVariant, string> = {
  active:
    "bg-accent-amber-soft text-accent-amber border-accent-amber/20",
  inactive:
    "bg-bg-elevated text-status-inactive-text border-border-subtle",
  danger:
    "bg-accent-danger-soft text-accent-danger border-accent-danger/20",
};

export function UIBadge({
  variant = "active",
  children,
  className = "",
}: UIBadgeProps) {
  return (
    <span
      className={`
        inline-flex items-center gap-[var(--spacing-1)]
        px-[var(--spacing-2)] py-[2px]
        rounded-[var(--radius-sm)] border
        font-sans text-[11px] font-medium
        ${variantClasses[variant]} ${className}
      `}
    >
      {children}
    </span>
  );
}
