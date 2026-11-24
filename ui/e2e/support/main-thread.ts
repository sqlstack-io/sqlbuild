import type { Page } from '@playwright/test';

// How much main-thread work an interaction costs, from a Chrome trace: the page's own script time
// (JavaScript from the input on, without the style and layout it forces), and the time from the
// input to the end of the frame that draws its result. Data that arrives later isn't counted.

type TraceEvent = { name: string; ph: string; ts: number; dur?: number; pid: number; tid: number; args?: { name?: string; data?: { type?: string } } };

export type InteractionCost = {
	/** Ms of JavaScript the page ran for the input, forced style and layout left out. */
	script: number;
	/** Ms from the input to the end of the frame that draws the update. */
	toFrame: number;
};

/** Events in which the page's JavaScript runs. */
const SCRIPT = new Set(['FunctionCall', 'EventDispatch', 'TimerFire', 'FireAnimationFrame', 'RunMicrotasks', 'EvaluateScript', 'v8.callFunction']);
/** Browser work that script can force while it runs. */
const FORCED = new Set(['UpdateLayoutTree', 'Layout', 'RecalcStyle']);
/** The input events an interaction dispatches. */
const INPUTS = new Set(['mousedown', 'mouseup', 'click', 'keydown', 'keyup', 'keypress']);
/** Events that end a frame. */
const FRAME = new Set(['Paint']);

const end = (e: TraceEvent) => e.ts + (e.dur ?? 0);
/** Total length of `events` with overlaps counted once. */
function covered(events: TraceEvent[]): number {
	let total = 0;
	let until = -Infinity;
	for (const e of [...events].sort((a, b) => a.ts - b.ts)) {
		const from = Math.max(e.ts, until);
		if (end(e) > from) total += end(e) - from;
		until = Math.max(until, end(e));
	}
	return total;
}

/** The cost of `act`, counting what the main thread does from its first input for `window` ms. */
export async function interactionCost(page: Page, act: () => Promise<void>, window = 50): Promise<InteractionCost> {
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
	if (!input) return { script: 0, toFrame: 0 };
	const within = on.filter((e) => e.ts >= input.ts && e.ts < input.ts + window * 1000);
	const scripts = within.filter((e) => SCRIPT.has(e.name));
	const forced = within.filter((e) => FORCED.has(e.name) && scripts.some((s) => e.ts >= s.ts && end(e) <= end(s)));
	// The input's own handlers (the click or key that updates the page, usually the longest), then the
	// first frame drawn after them.
	const handled = within
		.filter((e) => e.name === 'EventDispatch' && INPUTS.has(e.args?.data?.type ?? ''))
		.reduce((longest, e) => ((e.dur ?? 0) > (longest.dur ?? 0) ? e : longest), input);
	const frame = on.filter((e) => FRAME.has(e.name) && e.ts >= end(handled)).sort((a, b) => a.ts - b.ts)[0];
	return {
		script: (covered(scripts) - covered(forced)) / 1000,
		toFrame: ((frame ? end(frame) : end(handled)) - input.ts) / 1000
	};
}

/** The middle of `values`. */
export const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
