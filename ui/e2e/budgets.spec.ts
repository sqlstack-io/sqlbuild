import { expect, test, type CDPSession, type Page } from '@playwright/test';

// Work budgets: what each interaction costs, counted rather than timed, so a slow or busy machine gives
// the same numbers as a fast one. Each gesture is a fixed number of input events, each followed by two
// awaited animation frames. Per gesture the suite counts Chrome's layouts and style recalculations
// (Performance.getMetrics), DOM mutation records (every Svelte re-render that touches the page), the
// DOM nodes and cards or rows on the page afterwards, and the mock client's reads. Budgets sit just
// above the measured values; the breakdown next to each says where the count comes from.

type Cost = { layouts: number; styles: number; mutations: number; nodes: number; drawn: number; reads: number };
type Budget = Cost;

const RATE = Number(process.env.BUDGET_CPU_RATE ?? '1');
const TRACE = !!process.env.BUDGET_TRACE;
// Not reads: `supports` asks which methods exist. The app shell polls these on a 30 s wall-clock timer
// whatever the page does, so whether one lands inside a gesture depends on when the gesture started.
const NOT_COUNTED = new Set(['supports', 'getWarehouseSync', 'listPlatformEvents', 'navCounts']);

async function frames(page: Page, count = 2) {
	await page.evaluate(
		(n) =>
			new Promise<void>((resolve) => {
				let left = n;
				const tick = () => (--left <= 0 ? resolve() : requestAnimationFrame(tick));
				requestAnimationFrame(tick);
			}),
		count
	);
}

async function counters(page: Page, cdp: CDPSession) {
	// Nodes counts detached nodes until they are collected, so collect first.
	await cdp.send('HeapProfiler.collectGarbage');
	const { metrics } = await cdp.send('Performance.getMetrics');
	const value = (name: string) => metrics.find((m) => m.name === name)?.value ?? 0;
	const page_ = await page.evaluate(() => {
		const w = window as unknown as { __sqbMutations?: number; __sqbMockReads?: Record<string, number> };
		return { mutations: w.__sqbMutations ?? 0, readsByName: { ...(w.__sqbMockReads ?? {}) } };
	});
	const reads = Object.entries(page_.readsByName)
		.filter(([name]) => !NOT_COUNTED.has(name))
		.reduce((a, [, n]) => a + n, 0);
	return { layouts: value('LayoutCount'), styles: value('RecalcStyleCount'), nodes: value('Nodes'), mutations: page_.mutations, reads, readsByName: page_.readsByName };
}

/**
 * Runs in the page before the app. While a gesture is counted (`__sqbHold(true)`), the graph canvas's
 * settle timer (`settleMs`) is held and run once when counting stops, and interval ticks (the
 * one-second clock, the shell's polls) are skipped. Otherwise how often either fires inside a gesture
 * depends on how fast the machine runs the steps, and the counts with it.
 */
function holdTimers(settleMs: number) {
	const realSetTimeout = window.setTimeout.bind(window);
	const realClearTimeout = window.clearTimeout.bind(window);
	const realSetInterval = window.setInterval.bind(window);
	let holding = false;
	let next = 1e9;
	const held = new Map<number, () => void>();
	window.setTimeout = ((handler: TimerHandler, ms?: number, ...args: unknown[]) => {
		if (!holding || ms !== settleMs || typeof handler !== 'function') return realSetTimeout(handler, ms, ...args);
		held.set(++next, () => handler(...args));
		return next;
	}) as typeof window.setTimeout;
	window.clearTimeout = ((id?: number) => {
		if (id === undefined || !held.delete(id)) realClearTimeout(id);
	}) as typeof window.clearTimeout;
	window.setInterval = ((handler: TimerHandler, ms?: number, ...args: unknown[]) =>
		realSetInterval(typeof handler === 'function' ? (...a: unknown[]) => holding || handler(...a) : handler, ms, ...args)) as typeof window.setInterval;
	(window as unknown as { __sqbHold: (on: boolean) => void }).__sqbHold = (on) => {
		holding = on;
		if (on) return;
		const due = [...held.values()];
		held.clear();
		for (const run of due) run();
	};
}

const hold = (page: Page, on: boolean) => page.evaluate((o) => (window as unknown as { __sqbHold: (on: boolean) => void }).__sqbHold(o), on);

/** Open `path`, wait for `ready`, start counting mutations and set the CPU rate. */
async function open(page: Page, path: string, ready: string): Promise<CDPSession> {
	await page.setViewportSize({ width: 1600, height: 900 });
	// The graph canvas's SETTLE_MS.
	await page.addInitScript(holdTimers, 200);
	await page.goto(path);
	await page.locator(ready).first().waitFor({ timeout: 30_000 });
	await page.waitForLoadState('networkidle');
	await frames(page, 10);
	await page.evaluate(() => {
		const w = window as unknown as { __sqbMutations: number; __sqbMutationKinds: Record<string, number> };
		w.__sqbMutations = 0;
		w.__sqbMutationKinds = {};
		const kind = (r: MutationRecord) => {
			const el = (r.target.nodeType === 1 ? r.target : r.target.parentElement) as Element | null;
			const cls =
				el
					?.getAttribute('class')
					?.split(/\s+/)
					.find((c) => !c.startsWith('svelte-')) ??
				el?.tagName.toLowerCase() ??
				'?';
			return `${r.type}${r.attributeName ? `:${r.attributeName}` : ''} ${cls}`;
		};
		new MutationObserver((records) => {
			w.__sqbMutations += records.length;
			for (const r of records) w.__sqbMutationKinds[kind(r)] = (w.__sqbMutationKinds[kind(r)] ?? 0) + 1;
		}).observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });
	});
	const cdp = await page.context().newCDPSession(page);
	await cdp.send('Performance.enable');
	await cdp.send('Emulation.setCPUThrottlingRate', { rate: RATE });
	return cdp;
}

/** The cost of `act`, which ends with the page settled; `drawn` counts `drawnSelector` afterwards. */
async function cost(page: Page, cdp: CDPSession, drawnSelector: string, act: () => Promise<void>): Promise<Cost> {
	await frames(page);
	if (TRACE) await page.evaluate(() => ((window as unknown as { __sqbMutationKinds: Record<string, number> }).__sqbMutationKinds = {}));
	const before = await counters(page, cdp);
	await hold(page, true);
	await act();
	await hold(page, false);
	await page.waitForLoadState('networkidle');
	await frames(page, 4);
	const after = await counters(page, cdp);
	const readNames = Object.entries(after.readsByName)
		.map(([name, n]) => [name, n - (before.readsByName[name] ?? 0)] as const)
		.filter(([, n]) => n > 0)
		.map(([name, n]) => `${name}×${n}`);
	if (readNames.length) console.info(`READS ${readNames.join(' ')}`);
	if (TRACE) {
		const kinds = await page.evaluate(() => (window as unknown as { __sqbMutationKinds: Record<string, number> }).__sqbMutationKinds);
		const top = Object.entries(kinds)
			.sort((a, b) => b[1] - a[1])
			.slice(0, 8);
		console.info(`MUTATIONS ${top.map(([k, n]) => `${k}×${n}`).join(' | ')}`);
	}
	return {
		layouts: after.layouts - before.layouts,
		styles: after.styles - before.styles,
		mutations: after.mutations - before.mutations,
		nodes: after.nodes,
		drawn: await page.locator(drawnSelector).count(),
		reads: after.reads - before.reads
	};
}

function judge(name: string, measured: Cost, budget: Budget) {
	console.info(`BUDGET ${name} rate=${RATE} ${JSON.stringify(measured)}`);
	const over = (Object.keys(budget) as (keyof Cost)[]).filter((k) => measured[k] > budget[k]).map((k) => `${k} ${measured[k]} > ${budget[k]}`);
	// Soft, so one test reports every gesture that is over, not only the first.
	expect.soft(over, `${name}: over its work budget`).toEqual([]);
}

async function wheel(page: Page, at: { x: number; y: number }, steps: number, delta: (i: number) => number) {
	await page.mouse.move(at.x, at.y);
	for (let i = 0; i < steps; i++) {
		await page.mouse.wheel(0, delta(i));
		await frames(page);
	}
}

async function drag(page: Page, from: { x: number; y: number }, steps: number) {
	await page.mouse.move(from.x, from.y);
	await page.mouse.down();
	for (let i = 1; i <= steps; i++) {
		await page.mouse.move(from.x + i * 6, from.y + i * 2);
		await frames(page);
	}
	await page.mouse.up();
}

/** A point on the canvas with no card under it. */
async function emptyPoint(page: Page): Promise<{ x: number; y: number }> {
	return page.evaluate(() => {
		const r = document.querySelector('.svelte-flow')!.getBoundingClientRect();
		for (let y = r.top + 30; y < r.bottom - 30; y += 20)
			for (let x = r.left + 80; x < r.right - 30; x += 20)
				if (!document.elementFromPoint(x, y)?.closest('.svelte-flow__node, .svelte-flow__controls, .svelte-flow__edge, .svelte-flow__panel')) return { x, y };
		return { x: r.left + 80, y: r.top + 30 };
	});
}

const CARD = '.svelte-flow__node';
const GRAPHS = [
	{ name: 'large group', path: '/jobs/warehouse_nightly?mock&still' },
	{ name: 'Lineage overview', path: '/lineage?mock&large&still' }
];

for (const { name, path } of GRAPHS) {
	test(`${name}: wheel zoom and drag-pan`, async ({ page }) => {
		test.setTimeout(240_000);
		const cdp = await open(page, path, `.graph-canvas:not(.unfitted) ${CARD}`);
		const box = (await page.locator('.svelte-flow').first().boundingBox())!;
		const centre = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
		judge(`${name} wheel zoom`, await cost(page, cdp, CARD, () => wheel(page, centre, 60, (i) => (i < 30 ? 100 : -100))), BUDGETS[`${name} wheel zoom`]);
		const empty = await emptyPoint(page);
		judge(`${name} drag-pan`, await cost(page, cdp, CARD, () => drag(page, empty, 40)), BUDGETS[`${name} drag-pan`]);
	});
}

const ROW = 'table.sb-table tbody tr:not(.spacer)';

test('Checks: second visit with thousands of checks, then scrolling the list', async ({ page }) => {
	test.setTimeout(180_000);
	const nav = (label: string) => page.locator('nav a', { hasText: label }).first();
	const cdp = await open(page, '/?mock&awkward&still', 'nav a');
	await nav('Checks').click();
	await page.locator(ROW).first().waitFor();
	await page.waitForLoadState('networkidle');
	await nav('Overview').click();
	await page.waitForLoadState('networkidle');
	await frames(page, 4);
	judge(
		'Checks second visit',
		await cost(page, cdp, ROW, async () => {
			await nav('Checks').click();
			await page.locator(ROW).first().waitFor();
		}),
		BUDGETS['Checks second visit']
	);
	const list = (await page.locator('.sb-table-scroll').first().boundingBox())!;
	judge('Checks list scroll', await cost(page, cdp, ROW, () => wheel(page, { x: list.x + list.width / 2, y: list.y + 200 }, 40, () => 400)), BUDGETS['Checks list scroll']);
});

test('Plan: switching sections and showing more on a large changed build', async ({ page }) => {
	test.setTimeout(180_000);
	const PLAN_ROW = '.pane .plan-row';
	const cdp = await open(page, `/plan?mock&large&still&cmd=${encodeURIComponent('sqb build --changed')}`, PLAN_ROW);
	for (const key of ['first_run', 'unchanged', 'changed']) {
		judge(
			`Plan section ${key}`,
			await cost(page, cdp, PLAN_ROW, async () => {
				await page.locator(`.index .entry[data-section="${key}"]`).click();
				await page.locator(`.pane[data-section="${key}"] .plan-row`).first().waitFor();
			}),
			BUDGETS[`Plan section ${key}`]
		);
	}
	await page.locator('.index .entry[data-section="unchanged"]').click();
	await page.locator(`.pane[data-section="unchanged"] .plan-row`).nth(49).waitFor();
	judge(
		'Plan show more',
		await cost(page, cdp, PLAN_ROW, async () => {
			await page.getByRole('button', { name: /^Show more/ }).click();
			await page.locator(PLAN_ROW).nth(99).waitFor();
		}),
		BUDGETS['Plan show more']
	);
});

/**
 * Runs in the page before the app. While `__sqbHoldReads(true)`, the mock client's replies (each a
 * `readMs` timer) are held, and run when holding stops, so a test can see the page before a read lands.
 */
function holdReads(readMs: number) {
	const realSetTimeout = window.setTimeout.bind(window);
	let holding = false;
	let next = 2e9;
	const held: (() => void)[] = [];
	window.setTimeout = ((handler: TimerHandler, ms?: number, ...args: unknown[]) => {
		if (!holding || ms !== readMs || typeof handler !== 'function') return realSetTimeout(handler, ms, ...args);
		held.push(() => handler(...args));
		return ++next;
	}) as typeof window.setTimeout;
	(window as unknown as { __sqbHoldReads: (on: boolean) => void }).__sqbHoldReads = (on) => {
		holding = on;
		if (on) return;
		for (const run of held.splice(0)) run();
	};
}

const holdMockReads = (page: Page, on: boolean) => page.evaluate((o) => (window as unknown as { __sqbHoldReads: (on: boolean) => void }).__sqbHoldReads(o), on);

const FOLDER_ROW = '[data-testid="folder-tree"] tbody tr[data-path], [data-testid="folder-tree"] tbody tr.leaf';
const ISSUE = 'section.health .issues .issue';

test('Overview and folder view on a 10,000-node project', async ({ page }) => {
	test.setTimeout(300_000);
	// The mock client's reply latency (`latencyMs`).
	await page.addInitScript(holdReads, 120);
	const cdp = await open(page, '/?mock&huge&still', ISSUE);
	const quiet = async () => {
		await page.waitForLoadState('networkidle');
		await frames(page, 4);
	};
	judge(
		'10K open the folder view',
		await cost(page, cdp, FOLDER_ROW, async () => {
			await page.getByTestId('all-folders').click();
			await page.locator(FOLDER_ROW).nth(19).waitFor();
		}),
		BUDGETS['10K open the folder view']
	);
	judge(
		'10K expand all',
		await cost(page, cdp, FOLDER_ROW, async () => {
			await page.getByRole('button', { name: 'Expand all' }).click();
			await page.locator('[data-testid="folder-tree"] tbody tr.leaf, [data-testid="folder-more"]').first().waitFor();
		}),
		BUDGETS['10K expand all']
	);
	await page.getByRole('button', { name: 'Collapse all' }).click();
	await quiet();
	judge(
		'10K open one folder',
		await cost(page, cdp, FOLDER_ROW, async () => {
			await page.getByTestId('folder-toggle').first().click();
			await page.locator('[data-testid="folder-toggle"][aria-expanded="true"]').first().waitFor();
		}),
		BUDGETS['10K open one folder']
	);
	await page.getByRole('button', { name: 'Expand all' }).click();
	await quiet();
	const tree = (await page.getByTestId('folder-tree').locator('tbody').boundingBox())!;
	judge(
		'10K opened tree scroll',
		await cost(page, cdp, FOLDER_ROW, () => wheel(page, { x: tree.x + tree.width / 2, y: Math.min(tree.y + 200, 800) }, 20, (i) => (i < 10 ? 400 : -400))),
		BUDGETS['10K opened tree scroll']
	);
	judge(
		'10K back to the Overview',
		await cost(page, cdp, ISSUE, async () => {
			await page.locator('nav a', { hasText: 'Overview' }).first().click();
			await page.locator(ISSUE).first().waitFor();
		}),
		BUDGETS['10K back to the Overview']
	);
	const stale = page.locator(ISSUE, { hasText: 'out of date' });
	let listedWhileReading = -1;
	judge(
		'10K acknowledge nodes out of date',
		await cost(page, cdp, ISSUE, async () => {
			// The Overview's read that follows is held: the item must leave the list without waiting for it.
			await holdMockReads(page, true);
			await stale.getByRole('button', { name: 'Acknowledge' }).click();
			await frames(page, 4);
			listedWhileReading = await stale.count();
			await holdMockReads(page, false);
			await page.waitForFunction(() => ![...document.querySelectorAll('section.health .issues .issue')].some((e) => e.textContent?.includes('out of date')));
		}),
		BUDGETS['10K acknowledge nodes out of date']
	);
	console.info(`BUDGET 10K acknowledged item still listed while the Overview reads again: ${listedWhileReading}`);
	expect.soft(listedWhileReading, 'an acknowledged item leaves the list at once, not after the Overview is read again').toBe(0);
});

// Measured on 10 runs each at 1× and 6× CPU (Chrome throttling), with the breakdown traced from the
// mutation records (`BUDGET_TRACE=1`). The graph's settle timer is held while a gesture is counted and
// run once at its end, so DOM changes and reads are the same on every run and get 2 of headroom.
// Layouts and style recalculations still move by a few, because Chrome batches them per frame however
// fast the machine draws frames, and the Checks page renders a little more at 6×; those budgets are the
// highest count seen plus the spread seen plus 2. A regression that costs one more render per frame adds
// 35 to 45 DOM changes to every gesture; windowing switched off draws 5,000 rows instead of 20.
const BUDGETS: Record<string, Budget> = {
	// 60 wheel steps: the viewport transform is written once per step that moves (35; the rest stop at
	// the zoom limit), the zoom buttons toggle 3 times, and the settle restyles 7 edge paths, redraws the
	// grid and sets the canvas classes. Measured layouts 2, styles 43–46, mutations 54, 2,501 nodes, 9 cards.
	'large group wheel zoom': { layouts: 4, styles: 50, mutations: 56, nodes: 2600, drawn: 12, reads: 0 },
	// 40 pointer moves: one transform write and about one restyle each, plus the settle.
	// Measured layouts 1, styles 53–56, mutations 51.
	'large group drag-pan': { layouts: 3, styles: 61, mutations: 53, nodes: 2600, drawn: 12, reads: 0 },
	// 45 moving steps, then the settle: canvas classes, the grid redraw and the compact level.
	// Measured layouts 1, styles 50–51, mutations 57, 3,313 nodes, 13 cards.
	'Lineage overview wheel zoom': { layouts: 3, styles: 54, mutations: 59, nodes: 3400, drawn: 16, reads: 0 },
	// 40 transform writes, 2 card classes and the settle. Measured layouts 0, styles 43–46, mutations 44.
	'Lineage overview drag-pan': { layouts: 2, styles: 51, mutations: 46, nodes: 3400, drawn: 16, reads: 0 },
	// One listChecks read (filter counts come from it), 20 rows drawn: 72 row insertions and 200 cell
	// class writes, the rest the navigation. Measured layouts 2–4, styles 15–22, mutations 350 at 1× and
	// 377 at 6×, 3,327–3,545 nodes (as garbage collection finds them).
	'Checks second visit': { layouts: 7, styles: 31, mutations: 410, nodes: 3800, drawn: 24, reads: 1 },
	// 40 wheel steps of 400 px: the window keeps 29 rows (the view and 8 either side); each step swaps
	// rows in and out (1,822 insertions and removals), rewrites the recycled cells' classes (11,500) and
	// the two spacers' heights (78), with one layout per step. Measured layouts 40–41, styles 81,
	// mutations 13,400–13,414, 4,083–4,301 nodes.
	'Checks list scroll': { layouts: 44, styles: 83, mutations: 13430, nodes: 4600, drawn: 32, reads: 0 },
	// Each section switch replaces the 50 rows it shows (100 row insertions and removals) and reads its
	// first page once; Changed came with the page, so it reads nothing. Measured the same in every run:
	// layouts 2/2/1, styles 5/6/5, mutations 115/115/111.
	'Plan section first_run': { layouts: 4, styles: 7, mutations: 117, nodes: 2500, drawn: 50, reads: 1 },
	'Plan section unchanged': { layouts: 4, styles: 8, mutations: 117, nodes: 2500, drawn: 50, reads: 1 },
	'Plan section changed': { layouts: 3, styles: 7, mutations: 113, nodes: 2850, drawn: 50, reads: 0 },
	// Show more reads the next page once and adds its 50 rows. Measured layouts 2, styles 8–9, mutations 54.
	'Plan show more': { layouts: 4, styles: 12, mutations: 56, nodes: 3400, drawn: 100, reads: 1 },
	// The 10,000-node project (`?huge`), measured the same way: DOM changes, nodes, rows and reads the
	// same in every run but one (141 against 140 DOM changes going back to the Overview at 6×). The folder
	// tree windows its rows like any long list; drawing every folder or node instead adds thousands of rows.
	// Opening the folder view reads the selection vocabulary once and draws 40 rows: 320 cell class writes,
	// 40 rows' label and wrap classes, the rest the tab switch. Measured layouts 3, styles 12–17.
	'10K open the folder view': { layouts: 5, styles: 24, mutations: 456, nodes: 7300, drawn: 44, reads: 1 },
	// Expand all swaps the window's rows for the expanded tree's (316 row insertions and removals) and
	// rewrites their cells' classes. Measured layouts 1, styles 11–16, mutations 590 since the selection
	// bar became one row and the tree's window one row taller.
	'10K expand all': { layouts: 3, styles: 23, mutations: 592, nodes: 5700, drawn: 28, reads: 0 },
	// Opening one folder inserts its 20 subfolder rows and rewrites the window's cell classes (440).
	// Measured layouts 1–2, styles 9–15.
	'10K open one folder': { layouts: 5, styles: 23, mutations: 469, nodes: 7500, drawn: 48, reads: 0 },
	// 20 wheel steps of 400 px over the expanded tree: like the Checks list, each step swaps rows in and out
	// of the window (2,142) and rewrites the recycled cells' classes (6,579), one layout per step.
	// Measured layouts 20, styles 45–53, mutations 9,000 (8,800 before the selection bar became one row
	// and the window one row taller).
	'10K opened tree scroll': { layouts: 22, styles: 63, mutations: 9002, nodes: 5700, drawn: 28, reads: 0 },
	// Reads the Overview, its timeline and the cost summary once each, and draws the attention card's 9
	// items and the timeline. Measured layouts 3–4, styles 22–31, mutations 140–141.
	'10K back to the Overview': { layouts: 7, styles: 42, mutations: 143, nodes: 4600, drawn: 12, reads: 3 },
	// The acknowledgement and the Overview's read that follows it; the item leaves the list at once (the
	// test holds that read back to check), so 6 DOM changes. Measured layouts 1, styles 6–7.
	'10K acknowledge nodes out of date': { layouts: 3, styles: 10, mutations: 8, nodes: 4700, drawn: 11, reads: 2 }
};
