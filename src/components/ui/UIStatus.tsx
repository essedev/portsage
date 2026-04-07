interface UIStatusProps {
  active: boolean;
  className?: string;
}

export function UIStatus({ active, className = "" }: UIStatusProps) {
  return (
    <span
      className={`
        inline-block w-2 h-2 rounded-full shrink-0
        ${active
          ? "bg-accent-amber shadow-[0_0_6px_var(--color-accent-amber-glow)]"
          : "bg-status-inactive"
        }
        ${className}
      `}
    />
  );
}
