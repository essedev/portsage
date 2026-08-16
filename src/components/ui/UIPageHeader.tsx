import { type ReactNode } from "react";
import { UIText } from "@/components/ui/UIText";
import { UIDivider } from "@/components/ui/UIDivider";

interface UIPageHeaderProps {
  title: string;
  /** One line under the title. Optional, but every current view has one. */
  subtitle?: ReactNode;
  /** Buttons pinned to the right of the title row. */
  actions?: ReactNode;
  /** Views with their own separators (project detail) can turn this off. */
  divider?: boolean;
}

/**
 * The heading every main view shares: title, one line of context, optional
 * actions, then a divider. Extracted because the four views had copied it by
 * hand and the copies had already drifted - the trash view shipped with a
 * Settings-sized heading and no page padding.
 */
export function UIPageHeader({
  title,
  subtitle,
  actions,
  divider = true,
}: UIPageHeaderProps) {
  return (
    <>
      <div className="flex items-start justify-between gap-[var(--spacing-3)]">
        <div className="flex flex-col gap-[var(--spacing-1)]">
          <UIText variant="title" as="h2">
            {title}
          </UIText>
          {subtitle && (
            <UIText variant="body" className="text-text-secondary">
              {subtitle}
            </UIText>
          )}
        </div>
        {actions && <div className="flex items-center gap-[var(--spacing-2)] shrink-0">{actions}</div>}
      </div>
      {divider && <UIDivider />}
    </>
  );
}
