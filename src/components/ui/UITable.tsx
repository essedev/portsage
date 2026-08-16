import { type ReactNode } from "react";
import { UIText } from "@/components/ui/UIText";

export interface UITableColumn<T> {
  /** Stable identity for the column. Also the React key for its cells. */
  key: string;
  /** Column label. Omit for icon / action columns that need no heading. */
  header?: string;
  /**
   * Tailwind width class (`w-16`, `w-32`). Omit on exactly one column: that
   * one takes the remaining space. Widths live here and nowhere else, which
   * is the whole point - the hand-rolled tables kept a copy of them in the
   * header row and in the body row, and the copies drifted.
   */
  width?: string;
  align?: "left" | "center" | "right";
  /** Renders the cell. Gets the row, and whether the pointer is over it. */
  cell: (row: T) => ReactNode;
}

interface UITableProps<T> {
  columns: UITableColumn<T>[];
  rows: T[];
  rowKey: (row: T) => string | number;
  /** Shown instead of the table when there are no rows. */
  empty?: ReactNode;
  /** Hide the header row (short lists where the labels are noise). */
  hideHeader?: boolean;
  /** 32px rows instead of 36px, for dense lists like a project's ports. */
  dense?: boolean;
  /** Whole-row click. Rows become buttons for the keyboard when set. */
  onRowClick?: (row: T) => void;
  /** Extra classes for a specific row, e.g. to dim an inactive one. */
  rowClassName?: (row: T) => string;
}

/**
 * The one table in the app. A real `<table>` rather than nested flex rows:
 * `table-fixed` plus a `<colgroup>` means a column's width is declared once
 * and the header and every cell inherit it, which is exactly what the four
 * hand-written versions could not guarantee.
 *
 * Visually it is a contained object: bordered, rounded, with a tinted header
 * strip and hairline separators between rows. Bare rows floating on the page
 * background read as a list that happens to have labels; a frame tells you
 * where the data starts and ends.
 *
 * Row actions: put them in their own column with a fixed width and no
 * header, and give the button `opacity-0 group-hover:opacity-100` - every
 * row is a `group`. Reserving the width keeps rows aligned whether or not a
 * given row has an action.
 */
export function UITable<T>({
  columns,
  rows,
  rowKey,
  empty,
  hideHeader = false,
  dense = false,
  onRowClick,
  rowClassName,
}: UITableProps<T>) {
  if (rows.length === 0 && empty) {
    // Same frame as the populated table: an empty state that drops the
    // border makes the page jump the moment the first row arrives.
    return (
      <div className="rounded-[var(--radius-md)] border border-border-subtle bg-bg-surface px-[var(--spacing-3)] py-[var(--spacing-4)]">
        <UIText variant="body" className="text-text-muted">
          {empty}
        </UIText>
      </div>
    );
  }

  const alignClass = (align?: UITableColumn<T>["align"]) =>
    align === "right" ? "text-right" : align === "center" ? "text-center" : "text-left";
  // Centered columns hold icons and 24px action buttons: the 12px text
  // padding would squeeze them. Everything else gets the roomy padding that
  // keeps the frame from feeling cramped.
  const padClass = (align?: UITableColumn<T>["align"]) =>
    align === "center" ? "px-[var(--spacing-1)]" : "px-[var(--spacing-3)]";
  const rowHeight = dense ? "h-8" : "h-9";

  return (
    // overflow-hidden is what makes the corners actually round: without it
    // the header strip and the first row square them off again.
    <div className="rounded-[var(--radius-md)] border border-border-subtle overflow-hidden bg-bg-surface">
      <table className="w-full table-fixed border-collapse">
        <colgroup>
          {columns.map((c) => (
            <col key={c.key} className={c.width} />
          ))}
        </colgroup>
        {!hideHeader && (
          <thead>
            <tr className="bg-bg-elevated border-b border-border-subtle">
              {columns.map((c) => (
                <th
                  key={c.key}
                  scope="col"
                  className={`
                    h-8 ${padClass(c.align)}
                    font-sans text-[11px] font-medium uppercase tracking-wide text-text-secondary
                    ${alignClass(c.align)}
                  `}
                >
                  {c.header ?? ""}
                </th>
              ))}
            </tr>
          </thead>
        )}
        <tbody>
          {rows.map((row) => (
            <tr
              key={rowKey(row)}
              onClick={onRowClick ? () => onRowClick(row) : undefined}
              className={`
                group ${rowHeight} transition-colors duration-150
                border-b border-border-subtle/60 last:border-b-0
                hover:bg-bg-elevated
                ${onRowClick ? "cursor-pointer" : ""}
                ${rowClassName?.(row) ?? ""}
              `}
            >
              {columns.map((c) => (
                <td
                  key={c.key}
                  className={`${padClass(c.align)} ${alignClass(c.align)}`}
                >
                  {c.cell(row)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
