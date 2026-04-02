import { type InputHTMLAttributes } from "react";

interface GrimInputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  wrapperClassName?: string;
}

export function GrimInput({
  label,
  className = "",
  wrapperClassName = "",
  ...props
}: GrimInputProps) {
  return (
    <div className={`flex flex-col gap-[var(--spacing-1)] ${wrapperClassName}`}>
      {label && (
        <label className="font-sans text-[11px] font-medium text-text-secondary">
          {label}
        </label>
      )}
      <input
        className={`
          bg-bg-input border border-border-subtle rounded-[var(--radius-sm)]
          px-[var(--spacing-2)] py-[var(--spacing-1)] h-[30px]
          font-sans text-[13px] text-text-primary
          placeholder:text-text-muted
          focus:outline-none focus:border-accent-amber
          transition-colors duration-150
          ${className}
        `}
        {...props}
      />
    </div>
  );
}
