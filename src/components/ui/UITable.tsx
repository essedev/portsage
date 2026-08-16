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
    return (
      <UIText variant="body" className="text-text-muted">
        {empty}
      </UIText>
    );
  }

  const alignClass = (align?: UITableColumn<T>["align"]) =>
    align === "right" ? "text-right" : align === "center" ? "text-center" : "text-left";
  const rowHeight = dense ? "h-8" : "h-9";

  return (
    <table className="w-full table-fixed border-collapse">
      <colgroup>
        {columns.map((c) => (
          <col key={c.key} className={c.width} />
        ))}
      </colgroup>
      {!hideHeader && (
        <thead>
          <tr className="border-b border-border-subtle">
            {columns.map((c) => (
              <th
                key={c.key}
                scope="col"
                className={`
                  h-7 px-[var(--spacing-1)] pb-[var(--spacing-2)] align-bottom
                  font-sans text-[11px] font-medium text-text-secondary
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
              hover:bg-bg-elevated
              ${onRowClick ? "cursor-pointer" : ""}
              ${rowClassName?.(row) ?? ""}
            `}
          >
            {columns.map((c) => (
              <td
                key={c.key}
                className={`px-[var(--spacing-1)] ${alignClass(c.align)} first:rounded-l-[var(--radius-sm)] last:rounded-r-[var(--radius-sm)]`}
              >
                {c.cell(row)}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
