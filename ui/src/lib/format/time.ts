const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** Compact age, e.g. `just now`, `40s ago`, `4m ago`, `3h ago`, `2d ago`. */
export function formatAgo(iso: string | null | undefined, now: number): string {
	if (!iso) return '—';
	const diff = now - Date.parse(iso);
	// The clock ticks once a second and server clocks differ a little, so a moment ago can read a few seconds ahead.
	if (diff < -10 * SECOND) return formatIn(iso, now);
	if (diff < 10 * SECOND) return 'just now';
	// Seconds move in steps of ten, so a ticking label changes rarely.
	if (diff < MINUTE) return `${Math.floor(diff / (10 * SECOND)) * 10}s ago`;
	if (diff < HOUR) return `${Math.floor(diff / MINUTE)}m ago`;
	if (diff < DAY) return `${Math.floor(diff / HOUR)}h ago`;
	return `${Math.floor(diff / DAY)}d ago`;
}

/** Compact time until a future instant, e.g. `in 6m`. */
export function formatIn(iso: string | null | undefined, now: number): string {
	if (!iso) return '—';
	const diff = Date.parse(iso) - now;
	if (diff <= 0) return 'now';
	if (diff < MINUTE) return `in ${Math.ceil(diff / (10 * SECOND)) * 10}s`;
	if (diff < HOUR) return `in ${Math.round(diff / MINUTE)}m`;
	if (diff < DAY) return `in ${Math.round(diff / HOUR)}h`;
	return `in ${Math.round(diff / DAY)}d`;
}

/** Elapsed seconds since an instant, floored at zero. */
export function secondsSince(iso: string | null | undefined, now: number): number {
	if (!iso) return 0;
	return Math.max(0, Math.floor((now - Date.parse(iso)) / SECOND));
}

/** Duration between two instants, e.g. `1.4s`, `3m 12s`, `1h 5m`. */
export function formatDuration(startIso: string | null, endIso: string | null, now: number): string {
	if (!startIso) return '—';
	const ms = (endIso ? Date.parse(endIso) : now) - Date.parse(startIso);
	if (ms < MINUTE) return `${(ms / SECOND).toFixed(ms < 10 * SECOND ? 1 : 0)}s`;
	if (ms < HOUR) return `${Math.floor(ms / MINUTE)}m ${Math.floor((ms % MINUTE) / SECOND)}s`;
	return `${Math.floor(ms / HOUR)}h ${Math.floor((ms % HOUR) / MINUTE)}m`;
}

/** Local `YYYY-MM-DD HH:MM` for exact-time tooltips. */
export function formatStamp(iso: string | null | undefined): string {
	if (!iso) return '—';
	const d = new Date(iso);
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
