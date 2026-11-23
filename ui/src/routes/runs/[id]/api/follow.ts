// Reading a run's events and output on from where the last read stopped. Side effects: the client.

import type { ApiClient, RunEvent, RunOutputLine, Scope } from '$lib/api';

/** A read under way, and whether the run had ended when it started. */
type Reading = { ended: boolean; done: Promise<void> };

/** How far one run's events and output have been read. */
export type RunFollow = {
	events: RunEvent[];
	/** The last event's storage order, to read on after. */
	cursor: number | null;
	eventsDone: boolean;
	lines: (RunOutputLine & { runId: string })[];
	/** The last output line read, in the run's numbering; null before the first read. */
	lastLine: number | null;
	logs: RunEvent[];
	outputDone: boolean;
	/** The reads under way: one at most of the events and one of the output, as each appends from the cursor it started at. */
	reading: { events?: Reading; output?: Reading };
};

/** Pages of events one refresh reads at most for a run, so a long catch-up is spread over refreshes. */
const EVENT_PAGES = 5;
const EVENT_PAGE = 5000;

export function newFollow(): RunFollow {
	return { events: [], cursor: null, eventsDone: false, lines: [], lastLine: null, logs: [], outputDone: false, reading: {} };
}

/**
 * Runs `read` unless one is under way, which the caller then shares. A caller that saw the run ended waits out
 * a read started before that, whether it works or fails, and then reads once more for the run's last part.
 */
async function readAlone(follow: RunFollow, what: 'events' | 'output', ended: boolean, read: () => Promise<void>): Promise<void> {
	while (!follow[`${what}Done`]) {
		const under = follow.reading[what];
		if (!under) {
			const done = read().finally(() => delete follow.reading[what]);
			follow.reading[what] = { ended, done };
			return done;
		}
		if (under.ended || !ended) return under.done;
		await under.done.catch(() => {});
	}
}

/** The run's events: those already read and any since; `ended` means the run had ended when the read started. */
export async function followEvents(client: ApiClient, scope: Scope, follow: RunFollow, { id, ended }: { id: string; ended: boolean }): Promise<RunEvent[]> {
	await readAlone(follow, 'events', ended, async () => {
		for (let page = 0; page < EVENT_PAGES; page++) {
			const read = await client.listRunEvents(scope, id, { after: follow.cursor ?? undefined, limit: EVENT_PAGE });
			follow.events.push(...read.events);
			follow.cursor = read.events.reduce((last, e) => Math.max(last, e.storageId), follow.cursor ?? 0) || follow.cursor;
			if (read.nextCursor == null) {
				follow.eventsDone = ended;
				break;
			}
		}
	});
	return follow.events;
}

/** The run's printed lines: the newest at first, then those since; Recall's whole output where the server keeps no output store. */
export async function followOutput(client: ApiClient, scope: Scope, follow: RunFollow, { id, ended }: { id: string; ended: boolean }): Promise<RunFollow['lines']> {
	await readAlone(follow, 'output', ended, async () => {
		const read = client.followRunOutput ? await client.followRunOutput(scope, id, follow.lastLine) : null;
		if (read) {
			const lines = read.lines.map((line) => ({ ...line, runId: id }));
			follow.lines = read.replaced ? lines : [...follow.lines, ...lines];
			follow.lastLine = read.last;
			follow.logs = read.logs;
		} else {
			follow.lines = (await client.listRunOutput(scope, id)).map((line) => ({ ...line, runId: id }));
		}
		follow.outputDone = ended;
	});
	return follow.lines;
}
