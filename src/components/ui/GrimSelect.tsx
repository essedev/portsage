import { useState, useRef, useEffect } from "react";
import { ChevronDown } from "lucide-react";

interface GrimSelectOption {
  value: string;
  label: string;
}

interface GrimSelectProps {
  label?: string;
  options: GrimSelectOption[];
  value: string;
  onChange: (value: string) => void;
  className?: string;
}

export function GrimSelect({
  label,
  options,
  value,
  onChange,
  className = "",
}: GrimSelectProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const selected = options.find((o) => o.value === value);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  return (
    <div className={`flex flex-col gap-[var(--spacing-1)] ${className}`}>
      {label && (
        <label className="font-sans text-[11px] font-medium text-text-secondary">
          {label}
        </label>
      )}
      <div ref={ref} className="relative">
        <button
          type="button"
          onClick={() => setOpen(!open)}
          className="
            w-full flex items-center justify-between
            bg-bg-input border border-border-subtle rounded-[var(--radius-sm)]
            px-[var(--spacing-2)] h-[30px]
            font-mono text-[12px] text-text-primary
            focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-amber
            transition-colors duration-150
            cursor-pointer
          "
        >
          <span>{selected?.label ?? ""}</span>
          <ChevronDown
            size={14}
            className={`
              text-text-muted transition-transform duration-150
              ${open ? "rotate-180" : ""}
            `}
          />
        </button>

        {open && (
          <div
            className="
              absolute z-50 mt-1 w-full
              bg-bg-surface border border-border-subtle
              rounded-[var(--radius-sm)]
              shadow-lg shadow-black/30
              max-h-40 overflow-y-auto
            "
          >
            {options.map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => {
                  onChange(option.value);
                  setOpen(false);
                }}
                className={`
                  w-full text-left px-[var(--spacing-2)] py-[var(--spacing-1)]
                  font-mono text-[12px] cursor-pointer
                  transition-colors duration-150
                  focus:outline-none focus:bg-bg-elevated
                  ${option.value === value
                    ? "text-accent-amber bg-accent-amber-soft"
                    : "text-text-primary hover:bg-bg-elevated"
                  }
                `}
              >
                {option.label}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
