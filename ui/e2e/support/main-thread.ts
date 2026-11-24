import type { Page } from '@playwright/test';

// How long the browser's main thread is busy with an interaction, from a Chrome trace: each task
// the renderer's main thread runs from the input on, for `window` ms, which covers the input's
// handlers, the page's update and the frame that draws it, but not data that arrives later.

type TraceEvent = { name: string; ph: string; ts: number; dur?: number; pid: number; tid: number; args?: { name?: string; data?: { type?: string } } };

/** Lengths in ms of the main-thread tasks from the first input event `act` dispatches on, longest first. */
export async function interactionTasks(page: Page, act: () => Promise<void>, window = 50): Promise<number[]> {
	const cdp = await page.context().newCDPSession(page);
	const events: TraceEvent[] = [];
	cdp.on('Tracing.dataCollected', ({ value }) => events.push(...(value as TraceEvent[])));
	const done = new Promise<void>((resolve) => cdp.once('Tracing.tracingComplete', () => resolve()));
	await cdp.send('Tracing.start', { categories: 'devtools.timeline,disabled-by-default-devtools.timeline', transferMode: 'ReportEvents' });
	await act();
	await page.waitForTimeout(window + 250);
	await cdp.send('Tracing.end');
	await done;
	await cdp.detach();
	const main = new Set(events.filter((e) => e.name === 'thread_name' && e.args?.name === 'CrRendererMain').map((e) => `${e.pid}:${e.tid}`));
	const on = events.filter((e) => e.ph === 'X' && main.has(`${e.pid}:${e.tid}`));
	const input = on.filter((e) => e.name === 'EventDispatch' && ['mousedown', 'keydown', 'click'].includes(e.args?.data?.type ?? '')).sort((a, b) => a.ts - b.ts)[0];
	if (!input) return [];
	const tasks = on.filter((e) => e.name === 'RunTask');
	const start = tasks.find((t) => t.ts <= input.ts && input.ts <= t.ts + (t.dur ?? 0))?.ts ?? input.ts;
	return tasks
		.filter((t) => t.ts >= start && t.ts < start + window * 1000)
		.map((t) => (t.dur ?? 0) / 1000)
		.sort((a, b) => b - a);
}

/** The middle of `values`. */
export const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
