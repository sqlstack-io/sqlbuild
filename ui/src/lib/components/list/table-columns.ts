// Column definitions for the shared list table, and the sizing rules every list follows. Pure; no side effects.

/** One column of a list table. */
export type TableColumn = {
	id: string;
	/** Header text; may be empty for a narrow column such as a checkbox. */
	label: string;
	/** A fixed width such as `140px`, or a share such as `30%`; columns without one share what is left. */
	width?: string;
	/** The least a column without a fixed width may shrink to, in pixels, before the table scrolls sideways. */
	minWidth?: number;
	align?: 'left' | 'right';
	/** Keep the cell on one line, cut with an ellipsis and the full text on hover. On by default. */
	truncate?: boolean;
	/** Header tooltip. */
	title?: string;
	/** A class for the column's header and body cells, for a page's own styling. */
	class?: string;
};

/** Below this width every list scrolls sideways rather than squeezing its columns; defined once for every list. */
export const TABLE_MIN_WIDTH = 880;

/** Above this many rows a list draws only the rows in view. */
export const WINDOW_ABOVE = 200;

/** A column without a fixed width or a minimum keeps at least this much room. */
const FLEX_MIN = 120;

/** The table's minimum width: the shared minimum, or what its columns need when that is more. */
export function tableMinWidth(columns: readonly TableColumn[], base = TABLE_MIN_WIDTH): number {
	const need = columns.reduce((sum, c) => {
		const px = c.width?.endsWith('px') ? Number.parseFloat(c.width) : null;
		return sum + (px ?? c.minWidth ?? FLEX_MIN);
	}, 0);
	return Math.max(base, Math.ceil(need));
}

/** Whether a list of this many rows is windowed. */
export const windowed = (count: number) => count > WINDOW_ABOVE;

/** The classes a body cell takes from its column. */
export function cellClasses(column: TableColumn | undefined): string[] {
	if (!column) return [];
	return [column.truncate === false ? 'wrap' : 'trunc', ...(column.align === 'right' ? ['num'] : []), ...(column.class ? column.class.split(/\s+/) : [])];
}

/** "12 of 40" for a list's toolbar, or just "40" when nothing is filtered out. */
export const ofTotal = (shown: number, total: number) => (shown === total ? total.toLocaleString('en') : `${shown.toLocaleString('en')} of ${total.toLocaleString('en')}`);
