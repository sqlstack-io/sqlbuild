// Full local timestamps for the log and the drawer. Pure.

/** A full local timestamp with milliseconds, such as `2026-11-20 09:22:51.204`. */
export function fullStamp(iso: string): string {
	const d = new Date(iso);
	const p = (n: number, w = 2) => String(n).padStart(w, '0');
	return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}.${p(d.getMilliseconds(), 3)}`;
}
