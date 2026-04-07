import { type ReactNode } from "react";

type Variant = "title" | "section" | "body" | "label" | "mono";

interface UITextProps {
  variant?: Variant;
  className?: string;
  children: ReactNode;
  as?: keyof HTMLElementTagNameMap;
  style?: React.CSSProperties;
}

const variantClasses: Record<Variant, string> = {
  title:
    "font-mono text-[18px] text-accent-amber",
  section:
    "font-mono text-[13px] text-accent-amber",
  body: "font-sans text-[13px] text-text-primary",
  label: "font-sans text-[11px] font-medium text-text-secondary",
  mono: "font-mono text-[12px] text-text-primary",
};

export function UIText({
  variant = "body",
  className = "",
  children,
  as: Tag = "span",
  style,
}: UITextProps) {
  return (
    <Tag className={`${variantClasses[variant]} ${className}`} style={style}>
      {children}
    </Tag>
  );
}
