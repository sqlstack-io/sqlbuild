import { expect, test, type Page } from '@playwright/test';

// Timing guards. They run after the layout suite, alone (see playwright.config.ts), so other tests
// competing for the machine don't blur the timings.

async function settle(page: Page) {
	await page.waitForLoadState('networkidle');
	await page.waitForTimeout(500);
}

test.use({ viewport: { width: 1440, height: 1000 } });
// One at a time: timings taken side by side would measure each other.
test.describe.configure({ mode: 'serial' });

test('Checks opens again quickly from the sidebar with thousands of checks', async ({ page }) => {
	const nav = (name: string) => page.locator('nav a', { hasText: name }).first();
	const openChecks = async () => {
		const started = Date.now();
		await nav('Checks').click();
		await page.locator('table.sb-table tbody tr:not(.spacer)').first().waitFor();
		await page.waitForLoadState('networkidle');
		return Date.now() - started;
	};
	await page.goto('/?mock&awkward');
	await settle(page);
	await openChecks();
	await nav('Overview').click();
	await settle(page);
	// The first visit loads the list; the second took 12–19 s on a large project when filter options
	// were counted by filtering the whole list once per option.
	expect(await openChecks(), 'ms for the second visit to Checks to settle').toBeLessThan(2000);
});

// Panning and zooming a graph on a slow computer (the CPU slowed 6×): every gesture runs as a
// transform of the viewport's layer, so frames stay smooth. Wheel zoom with a mouse, a trackpad's
// two-finger scroll and its momentum tail (many small wheel events, which the canvas reads as zoom),
// pinch (ctrl + wheel), and drag-panning from empty canvas and from a card. Each gesture runs three
// times and the run with the median worst frame is judged, so a one-off stall of a shared machine
// doesn't decide it, while a cost every gesture pays does.

type Frames = { p90: number; max: number };
const P90_MS = 50;
const MAX_MS = 200;

/** Frame times while `act` runs, from requestAnimationFrame. */
async function frameTimes(page: Page, act: () => Promise<void>): Promise<Frames> {
	await page.evaluate(() => {
		const w = window as unknown as { __frames: number[]; __measuring: boolean };
		w.__frames = [];
		w.__measuring = true;
		let last = performance.now();
		const tick = (t: number) => {
			w.__frames.push(t - last);
			last = t;
			if (w.__measuring) requestAnimationFrame(tick);
		};
		requestAnimationFrame(tick);
	});
	await act();
	await page.waitForTimeout(300);
	const frames = await page.evaluate(() => {
		const w = window as unknown as { __frames: number[]; __measuring: boolean };
		w.__measuring = false;
		return w.__frames.slice(1);
	});
	const sorted = [...frames].sort((a, b) => a - b);
	return { p90: sorted[Math.floor(sorted.length * 0.9)] ?? 0, max: sorted[sorted.length - 1] ?? 0 };
}

async function medianRun(page: Page, act: () => Promise<void>): Promise<Frames> {
	const runs: Frames[] = [];
	for (let i = 0; i < 3; i++) runs.push(await frameTimes(page, act));
	return runs.sort((a, b) => a.max - b.max)[1];
}

/** A point on the canvas with no card under it, for panning from empty canvas. */
async function emptyPoint(page: Page): Promise<{ x: number; y: number }> {
	return page.evaluate(() => {
		const r = document.querySelector('.svelte-flow')!.getBoundingClientRect();
		for (let y = r.top + 30; y < r.bottom - 30; y += 20)
			for (let x = r.left + 80; x < r.right - 30; x += 20) if (!document.elementFromPoint(x, y)?.closest('.svelte-flow__node, .svelte-flow__controls, .svelte-flow__edge')) return { x, y };
		return { x: r.left + 80, y: r.top + 30 };
	});
}

const GRAPHS = [
	{ name: 'the large group', path: '/jobs/warehouse_nightly?mock', panFromCard: true },
	// Lineage's cards are dragged, not panned from.
	{ name: 'the Lineage overview', path: '/lineage?mock&large', panFromCard: false }
];

for (const { name, path, panFromCard } of GRAPHS) {
	test(`${name} pans and zooms smoothly on a slow computer`, async ({ page }) => {
		test.setTimeout(240_000);
		await page.setViewportSize({ width: 1600, height: 900 });
		await page.goto(path);
		await page.locator('.graph-canvas:not(.unfitted) .svelte-flow__node').first().waitFor({ timeout: 20_000 });
		await page.waitForTimeout(800);
		const box = (await page.locator('.svelte-flow').first().boundingBox())!;
		const centre = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
		const empty = await emptyPoint(page);
		const card = (await page.locator('.svelte-flow__node').first().boundingBox())!;
		const cdp = await page.context().newCDPSession(page);
		await cdp.send('Emulation.setCPUThrottlingRate', { rate: 6 });

		const wheel = async (steps: number, delta: (i: number) => number, gapMs: number) => {
			await page.mouse.move(centre.x, centre.y);
			for (let i = 0; i < steps; i++) {
				await page.mouse.wheel(0, delta(i));
				await page.waitForTimeout(gapMs);
			}
		};
		const drag = async (from: { x: number; y: number }) => {
			await page.mouse.move(from.x, from.y);
			await page.mouse.down();
			for (let i = 1; i <= 30; i++) {
				await page.mouse.move(from.x + i * 6, from.y + i * 2);
				await page.waitForTimeout(16);
			}
			for (let i = 29; i >= 0; i--) await page.mouse.move(from.x + i * 6, from.y + i * 2);
			await page.mouse.up();
		};
		const gestures: [string, () => Promise<void>][] = [
			['wheel zoom', () => wheel(60, (i) => (i < 30 ? 100 : -100), 16)],
			['trackpad momentum', () => wheel(60, (i) => Math.max(1, 40 * Math.pow(0.94, i)), 8)],
			[
				'pinch',
				async () => {
					await page.keyboard.down('Control');
					await wheel(30, (i) => (i < 15 ? 6 : -6), 8);
					await page.keyboard.up('Control');
				}
			],
			['drag-pan from empty canvas', () => drag(empty)],
			...(panFromCard ? ([['drag-pan from a card', () => drag({ x: card.x + 40, y: card.y + 15 })]] as [string, () => Promise<void>][]) : [])
		];
		const results: string[] = [];
		const over: string[] = [];
		for (const [gesture, act] of gestures) {
			const f = await medianRun(page, act);
			results.push(`${gesture}: p90 ${Math.round(f.p90)} ms, max ${Math.round(f.max)} ms`);
			// Frames last whole multiples of about 16.7 ms: three frames (50.05 ms) is within 50 ms.
			if (f.p90 > P90_MS + 1 || f.max > MAX_MS + 1) over.push(`${gesture}: p90 ${Math.round(f.p90)} ms, max ${Math.round(f.max)} ms`);
		}
		console.info(`${name}, CPU slowed 6×: ${results.join('; ')}`);
		expect(over, `frames over budget (p90 ≤ ${P90_MS} ms, max ≤ ${MAX_MS} ms)`).toEqual([]);
	});
}
