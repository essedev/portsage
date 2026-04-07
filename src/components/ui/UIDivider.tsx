interface UIDividerProps {
  className?: string;
}

export function UIDivider({ className = "" }: UIDividerProps) {
  return (
    <hr
      className={`border-0 border-t border-border-subtle ${className}`}
    />
  );
}
