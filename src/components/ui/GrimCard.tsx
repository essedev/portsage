import { type ReactNode } from "react";

interface GrimCardProps {
  children: ReactNode;
  className?: string;
  glow?: boolean;
  onClick?: () => void;
}

export function GrimCard({
  children,
  className = "",
  glow = false,
  onClick,
}: GrimCardProps) {
  return (
    <div
      onClick={onClick}
      className={`
        bg-bg-surface border border-border-subtle rounded-[var(--radius-md)]
        p-[var(--spacing-3)] transition-all duration-150
        ${glow ? "hover:border-accent-amber hover:shadow-[0_0_8px_var(--color-accent-amber-glow)]" : "hover:bg-bg-elevated"}
        ${onClick ? "cursor-pointer" : ""}
        ${className}
      `}
    >
      {children}
    </div>
  );
}
