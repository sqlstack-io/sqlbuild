// Pure helpers for the toolbar's choice filter (`ChoiceFilter`): matching options to a search,
// ordering, selecting what matches, truncating long labels in the middle, windowing and the URL
// form of a filter with several values.

export interface ChoiceOption<T extends string = string> {
	value: T;
	label: string;
	count?: number;
	/** Explains a short label on hover. */
	title?: string;
}

/** Fewer options than this need no search box. */
export const SEARCH_FROM = 10;
/** More options than this are drawn windowed, so thousands open instantly. */
export const WINDOW_ABOVE = 200;

export const showsSearch = (count: number) => count >= SEARCH_FROM;
export const isWindowed = (count: number) => count > WINDOW_ABOVE;

/** Whether `value` passes a filter set to `selected`: any of them, or anything when none is. */
export function matchesAny<T>(selected: readonly T[] | ReadonlySet<T>, value: T | null | undefined): boolean {
	const set = selected instanceof Set ? selected : new Set(selected as readonly T[]);
	return set.size === 0 || (value !== null && value !== undefined && set.has(value));
}

/** The options whose label contains `query` anywhere, ignoring case; all of them for a blank query. */
export function matchOptions<O extends { label: string }>(options: readonly O[], query: string): O[] {
	const q = query.trim().toLowerCase();
	return q ? options.filter((o) => o.label.toLowerCase().includes(q)) : [...options];
}

/** The options with those in `first` ahead of the rest, each part in its original order. */
export function selectedFirst<O extends { value: string }>(options: readonly O[], first: ReadonlySet<string>): O[] {
	if (!first.size) return [...options];
	return [...options.filter((o) => first.has(o.value)), ...options.filter((o) => !first.has(o.value))];
}

/** `selected` plus every value in `matches`, in option order after the ones already chosen. */
export function selectMatching<T extends string>(selected: readonly T[], matches: readonly { value: T }[]): T[] {
	const out = [...selected];
	const have = new Set(selected);
	for (const { value } of matches) {
		if (have.has(value)) continue;
		have.add(value);
		out.push(value);
	}
	return out;
}

/** `selected` without any value in `matches`. */
export function clearMatching<T extends string>(selected: readonly T[], matches: readonly { value: T }[]): T[] {
	const drop = new Set(matches.map((m) => m.value));
	return selected.filter((v) => !drop.has(v));
}

/** `selected` with `value` added, or removed when it is already there. */
export function toggleValue<T extends string>(selected: readonly T[], value: T): T[] {
	return selected.includes(value) ? selected.filter((v) => v !== value) : [...selected, value];
}

/** What the filter's button says after its label: nothing, the one choice, or how many. */
export function choiceSummary(selected: readonly string[], options: readonly ChoiceOption[]): string {
	if (!selected.length) return '';
	if (selected.length > 1) return `${selected.length} selected`;
	return options.find((o) => o.value === selected[0])?.label ?? selected[0];
}

const SEPARATORS = ['__', '.', '/', ':', '-', '_', ' '];
const LONGEST_TAIL = 16;

/**
 * A label split for truncating in the middle: the start shrinks behind an ellipsis while the tail
 * always shows, e.g. `orders__int_clean__…__emea`. The tail starts at the last separator that leaves
 * it at most 16 characters, or is the last 10 characters; short labels have no tail.
 */
export function splitMiddle(label: string): { head: string; tail: string } {
	if (label.length <= 2 * LONGEST_TAIL) return { head: label, tail: '' };
	const window = label.slice(-LONGEST_TAIL);
	let cut = -1;
	for (const sep of SEPARATORS) {
		const at = window.indexOf(sep);
		// The earliest separator in the window keeps the longest meaningful tail, as long as it isn't the very start.
		if (at > 0 && (cut === -1 || at < cut)) cut = at;
	}
	const start = cut === -1 ? label.length - 10 : label.length - LONGEST_TAIL + cut;
	return { head: label.slice(0, start), tail: label.slice(start) };
}

/** A width for the open filter: wide enough for its longest label and count, within 280 px and 520 px or 40% of the window. */
export function panelWidth(options: readonly ChoiceOption[], multiple: boolean): string {
	let chars = 0;
	let digits = 0;
	for (const o of options) {
		if (o.label.length > chars) chars = o.label.length;
		if (o.count != null) digits = Math.max(digits, String(o.count).length);
	}
	// Row padding and gaps, the checkbox, and the count column.
	const extra = 28 + (multiple ? 22 : 0) + (digits ? digits * 8 + 12 : 0);
	return `clamp(280px, calc(${chars}ch + ${extra}px), min(520px, 40vw))`;
}

/**
 * The values of a filter in a URL: repeated parameters (`?node=a&node=b`), each of which may also
 * hold several values separated by commas (`?node=a,b`). A link with one value, as before, gives one.
 */
export function readValues(params: URLSearchParams, key: string): string[] {
	const out: string[] = [];
	for (const raw of params.getAll(key)) for (const v of raw.split(',')) if (v && !out.includes(v)) out.push(v);
	return out;
}

/** `params` with the filter's values as one repeated parameter each, or without it when none is chosen. */
export function writeValues(params: URLSearchParams, key: string, values: readonly string[]): URLSearchParams {
	const next = new URLSearchParams(params);
	next.delete(key);
	for (const v of values) next.append(key, v);
	return next;
}
