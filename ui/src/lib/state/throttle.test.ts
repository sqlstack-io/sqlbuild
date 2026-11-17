import { describe, expect, it } from 'vitest';
import { throttled, type Timers } from './throttle';

/** Lets a finished load's handlers run. */
const settle = () => new Promise((done) => setImmediate(done));

/** Timers driven by hand: `advance` moves the clock and fires what is due. */
function fakeTimers() {
	let now = 0;
	let queue: { at: number; run: () => void; id: number }[] = [];
	let next = 0;
	const timers: Timers = {
		now: () => now,
		set: (run, ms) => (queue.push({ at: now + ms, run, id: ++next }), next),
		clear: (id) => (queue = queue.filter((t) => t.id !== id))
	};
	const advance = async (ms: number) => {
		const until = now + ms;
		for (;;) {
			queue.sort((a, b) => a.at - b.at);
			const due = queue[0];
			if (!due || due.at > until) break;
			queue.shift();
			now = due.at;
			due.run();
			await settle();
		}
		now = until;
	};
	return { timers, advance };
}

describe('a throttled reload', () => {
	it('starts at most once per interval under a stream of signals, and once more after the last', async () => {
		const { timers, advance } = fakeTimers();
		const starts: number[] = [];
		const reload = throttled(async () => void starts.push(timers.now()), 10_000, timers);
		// A signal every second for 25 seconds.
		for (let s = 0; s < 25; s++) {
			reload.request();
			await advance(1000);
		}
		await advance(20_000);
		expect(starts).toEqual([0, 10_000, 20_000, 30_000]);
	});

	it('never runs two loads at once; signals during a load ask for one more', async () => {
		const { timers, advance } = fakeTimers();
		let running = 0;
		let most = 0;
		let finish: () => void = () => {};
		const reload = throttled(
			() => {
				running++;
				most = Math.max(most, running);
				return new Promise<void>((done) => (finish = () => (running--, done())));
			},
			0,
			timers
		);
		reload.request();
		reload.request();
		reload.request();
		expect(running).toBe(1);
		finish();
		await settle();
		expect(running).toBe(1);
		finish();
		await settle();
		expect(running).toBe(0);
		expect(most).toBe(1);
	});
});
