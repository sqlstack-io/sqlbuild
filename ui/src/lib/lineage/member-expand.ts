// Which member jobs are open in place on a group's page. Pure; no side effects.

/** `open` with the member opened, or closed when it was open. */
export function toggled(open: ReadonlySet<string>, jobKey: string): Set<string> {
	const next = new Set(open);
	if (!next.delete(jobKey)) next.add(jobKey);
	return next;
}

/** `open` with the member open, as selecting it from elsewhere does. */
export function withOpen(open: ReadonlySet<string>, jobKey: string): Set<string> {
	return new Set([...open, jobKey]);
}
