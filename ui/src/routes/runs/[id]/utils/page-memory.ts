// What a run page showed, kept while the app is open, so Back from the output viewer, or
// any page a link opened, returns to the page exactly as it was: selection, filters, open rows, tab,
// drawer and scroll. It lives in memory only; a reload starts afresh.

const pages = new Map<string, Record<string, unknown>>();

/** The remembered state of one run page, written as it changes. */
export function pageMemory(runId: string): Record<string, unknown> {
	let memory = pages.get(runId);
	if (!memory) pages.set(runId, (memory = {}));
	return memory;
}
