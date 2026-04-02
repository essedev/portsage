interface GrimDividerProps {
  className?: string;
}

export function GrimDivider({ className = "" }: GrimDividerProps) {
  return (
    <hr
      className={`border-0 border-t border-border-subtle ${className}`}
    />
  );
}
