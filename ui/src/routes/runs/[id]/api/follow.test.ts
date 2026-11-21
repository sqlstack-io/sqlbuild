import { describe, expect, it } from 'vitest';
import type { ApiClient, RunEvent, RunOutputFollow, RunOutputLine, Scope } from '$lib/api';
import { followEvents, followOutput, newFollow } from './follow';

const scope = { locationId: 'loc', target: 'dev' } as unknown as Scope;
const event = (storageId: number) => ({ id: `e${storageId}`, storageId, occurredAt: `2026-11-21T09:00:0${storageId}Z` }) as RunEvent;
const line = (text: string) => ({ invocationId: '', stream: 'stdout', at: '2026-11-21T09:00:00Z', text, error: false }) as RunOutputLine;

/** A client that answers each read from a queue and records what it was asked. */
function client(answers: { events?: { events: RunEvent[]; nextCursor: number | null }[]; output?: (RunOutputFollow | null)[] }) {
	const asked: string[] = [];
	const fake = {
		listRunEvents: async (_: Scope, id: string, query: { after?: number }) => {
			asked.push(`events ${id} after ${query.after ?? '-'}`);
			return answers.events!.shift()!;
		},
		followRunOutput: async (_: Scope, id: string, after: number | null) => {
			asked.push(`output ${id} after ${after ?? '-'}`);
			return answers.output!.shift()!;
		},
		listRunOutput: async (_: Scope, id: string) => {
			asked.push(`recall output ${id}`);
			return [line('from recall')];
		}
	};
	return { fake: fake as unknown as ApiClient, asked };
}

describe('following a run', () => {
	it('reads events on from the last one, and stops once a read started after the run ended', async () => {
		const { fake, asked } = client({ events: [{ events: [event(1), event(2)], nextCursor: null }, { events: [event(3)], nextCursor: null }] });
		const follow = newFollow();
		await followEvents(fake, scope, follow, { id: 'r1', ended: false });
		await followEvents(fake, scope, follow, { id: 'r1', ended: true });
		const after = await followEvents(fake, scope, follow, { id: 'r1', ended: true });
		expect([after.map((e) => e.storageId), asked]).toEqual([
			[1, 2, 3],
			['events r1 after -', 'events r1 after 2']
		]);
	});

	it('reads the newest output first, then only new lines, and takes the newest again when told to', async () => {
		const logs = [event(9)];
		const { fake, asked } = client({
			output: [
				{ lines: [line('a'), line('b')], logs: [], last: 2, replaced: true },
				{ lines: [line('c')], logs, last: 3, replaced: false },
				{ lines: [line('z')], logs, last: 9000, replaced: true }
			]
		});
		const follow = newFollow();
		const reads = [];
		for (const ended of [false, false, true, true]) reads.push((await followOutput(fake, scope, follow, { id: 'r1', ended })).map((l) => l.text).join(''));
		expect([reads, asked, follow.logs]).toEqual([['ab', 'abc', 'z', 'z'], ['output r1 after -', 'output r1 after 2', 'output r1 after 3'], logs]);
	});

	it('reads Recall’s output where the server keeps no output store', async () => {
		const { fake, asked } = client({ output: [null] });
		const lines = await followOutput(fake, scope, newFollow(), { id: 'r1', ended: false });
		expect([lines.map((l) => [l.text, l.runId]), asked]).toEqual([[['from recall', 'r1']], ['output r1 after -', 'recall output r1']]);
	});
});
