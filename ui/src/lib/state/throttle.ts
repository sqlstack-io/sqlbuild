// A reload that a stream of change signals can ask for as often as it likes, but that starts at most
// once every `minMs`, never twice at a time, and always once more after the last signal. Pure apart
// from the timers it is given.

export type Timers = { now: () => number; set: (run: () => void, ms: number) => unknown; clear: (handle: unknown) => void };
const realTimers: Timers = { now: () => Date.now(), set: (run, ms) => setTimeout(run, ms), clear: (h) => clearTimeout(h as ReturnType<typeof setTimeout>) };

export type Throttled = { request: () => void; cancel: () => void };

/**
 * `load` run on `request()`: at once when idle and at least `minMs` after the last start, else once
 * later. Requests while a load runs, or while one waits, ask for just one more.
 */
export function throttled(load: () => Promise<unknown>, minMs: number, timers: Timers = realTimers): Throttled {
	let lastStart = -Infinity;
	let running = false;
	let again = false;
	let waiting: unknown = null;
	const start = () => {
		waiting = null;
		running = true;
		again = false;
		lastStart = timers.now();
		void load()
			.catch(() => {})
			.finally(() => {
				running = false;
				if (again) request();
			});
	};
	function request() {
		if (running) {
			again = true;
			return;
		}
		if (waiting !== null) return;
		const wait = lastStart + minMs - timers.now();
		if (wait <= 0) start();
		else waiting = timers.set(start, wait);
	}
	return {
		request,
		cancel() {
			if (waiting !== null) timers.clear(waiting);
			waiting = null;
			again = false;
		}
	};
}
