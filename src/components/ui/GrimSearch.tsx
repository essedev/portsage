import { Search } from "lucide-react";
import { type InputHTMLAttributes } from "react";

interface GrimSearchProps extends InputHTMLAttributes<HTMLInputElement> {}

export function GrimSearch({ className = "", ...props }: GrimSearchProps) {
  return (
    <div className={`relative ${className}`}>
      <Search
        size={16}
        className="absolute left-[var(--spacing-2)] top-1/2 -translate-y-1/2 text-text-muted"
      />
      <input
        className="
          w-full bg-bg-input border border-border-subtle
          rounded-[var(--radius-sm)]
          pl-[var(--spacing-8)] pr-[var(--spacing-2)] py-[var(--spacing-1)]
          font-sans text-[13px] text-text-primary
          placeholder:text-text-muted
          focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-amber
          transition-colors duration-150
        "
        placeholder="Search..."
        {...props}
      />
    </div>
  );
}
