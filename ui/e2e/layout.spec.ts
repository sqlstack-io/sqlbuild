import { expect, test, type Page } from '@playwright/test';

// Every page, with every list stretched to 5,000 rows and very long names (`?mock&awkward`), at two
// common desktop widths: nothing overflows the window sideways, header cells line up with their
// columns, long lists draw a bounded number of rows, and sticky headers cover the rows under them.

const WIDTHS = [1440, 1920];
/** Most rows a windowed list may have in the page at once. */
const MAX_ROWS = 400;

const PAGES = [
	'/',
	'/admin/audit',
	'/admin/check-access',
	'/admin/people',
	'/admin/roles',
	'/catalog',
	'/changes',
	'/checks',
	'/connections',
	'/cost',
	'/jobs',
	'/lineage',
	'/locations',
	'/maintenance',
	'/plan',
	'/providers',
	'/runs',
	'/settings',
	'/triggers'
];
/** Item pages, reached through the first row of their list. */
const ITEMS = [
	{ list: '/jobs', link: 'a[href*="/jobs/"]' },
	{ list: '/catalog', link: 'a[href*="/catalog/"]' },
	{ list: '/checks', link: 'a[href*="/checks/"]' },
	{ list: '/triggers', link: 'a[href*="/triggers/"]' },
	{ list: '/runs', link: 'a[href*="/runs/"]' },
	{ list: '/providers', link: 'a[href*="/providers/"]' },
	{ list: '/admin/roles', link: 'a[href*="/admin/roles/"]' }
];

/** The mock's large job group (src/lib/api/mock/big-group.ts). */
const BIG_GROUP = 'warehouse_nightly';

const awkward = (path: string) => `${path}${path.includes('?') ? '&' : '?'}mock&awkward`;

async function settle(page: Page) {
	await page.waitForLoadState('networkidle');
	await page.waitForTimeout(500);
	// A graph is drawn once its data loads and shows once its first fit is done; the layout check fails
	// one still hidden. Wait for every graph to have cards and be fitted, or to say it has too many to draw:
	// a canvas without cards yet isn't marked unfitted, so waiting for that alone could stop too early.
	await page
		.waitForFunction(
			() =>
				[...document.querySelectorAll('.graph-canvas')].every((c) => (c.querySelector('.svelte-flow__node') ? !c.classList.contains('unfitted') : !!c.parentElement?.querySelector('.graph-too-many'))),
			null,
			{ timeout: 30_000 }
		)
		.catch(() => {});
}

/** In the page: each node card's status line that wraps or cuts a part off, as problems. */
function statusLines(): string[] {
	const problems: string[] = [];
	// A node card's status line is one line, each part whole: none wraps, none is cut off.
	for (const line of document.querySelectorAll<HTMLElement>('.svelte-flow__node .status')) {
		const box = line.getBoundingClientRect();
		if (!box.height || getComputedStyle(line).visibility === 'hidden') continue;
		const name = line.closest<HTMLElement>('.svelte-flow__node')?.dataset.id;
		const parts = [...line.children].filter((c) => c.getBoundingClientRect().width > 0);
		// Wrapped: a run of a part's text over two lines (its line boxes, from a range over each text
		// node; icons beside the text sit at their own height), or parts on different rows.
		const lines = (p: Element) => {
			let most = 0;
			const walk = document.createTreeWalker(p, NodeFilter.SHOW_TEXT);
			for (let text = walk.nextNode(); text; text = walk.nextNode()) {
				if (!text.textContent?.trim()) continue;
				const range = document.createRange();
				range.selectNodeContents(text);
				most = Math.max(most, new Set([...range.getClientRects()].filter((r) => r.width > 0).map((r) => Math.round(r.top))).size);
			}
			return most;
		};
		const middles = parts.map((p) => p.getBoundingClientRect().top + p.getBoundingClientRect().height / 2);
		const wrapped = parts.some((p) => lines(p) > 1) || Math.max(...middles) - Math.min(...middles) > 3;
		const cut = parts.some((p) => p.getBoundingClientRect().right > box.right - 1);
		if (wrapped || cut || line.scrollWidth > line.clientWidth + 1) {
			problems.push(`${name}: its status line ${wrapped ? 'wraps' : 'is cut off'} ("${(line.textContent ?? '').replace(/\s+/g, ' ').trim()}")`);
		}
	}
	return problems;
}

/** In the page: each fitted graph's start card (`data-fit-anchor`) not wholly inside its canvas, as problems. */
function anchorOutside(): string[] {
	const found: string[] = [];
	for (const flow of document.querySelectorAll<HTMLElement>('.svelte-flow[data-fit-anchor]')) {
		const id = flow.dataset.fitAnchor;
		const card = id ? [...flow.querySelectorAll<HTMLElement>('.svelte-flow__node')].find((n) => n.dataset.id === id) : null;
		if (!card) continue;
		const [pane, box] = [flow.getBoundingClientRect(), card.getBoundingClientRect()];
		if (box.left < pane.left || box.right > pane.right || box.top < pane.top || box.bottom > pane.bottom) {
			found.push(`the card the graph opens on (${id}) is cut off: ${Math.round(box.left - pane.left)}px from the left, ${Math.round(box.top - pane.top)}px from the top`);
		}
		// Unless it centres on an open box, it starts on the leftmost column, so that column is shown whole.
		if (card.classList.contains('svelte-flow__node-folderBox') || card.classList.contains('svelte-flow__node-memberBox')) continue;
		// Nodes are siblings in the page; a box's contents start right of the box, so any node starting
		// further left than half a card is a column left of the start card.
		const further = [...flow.querySelectorAll<HTMLElement>('.svelte-flow__node')].find((n) => n.getBoundingClientRect().left < box.left - box.width / 2);
		if (further) found.push(`the graph opens beside its first column: ${further.dataset.id} starts ${Math.round(box.left - further.getBoundingClientRect().left)}px left of ${id}`);
	}
	return found;
}

/** The graph's pan and zoom, as Svelte Flow sets it on its viewport. */
const viewOf = (page: Page) => page.locator('.svelte-flow__viewport').first().evaluate((el) => (el as HTMLElement).style.transform);

/** Zooming in and pressing Reset comes back to the view the graph opened on. */
async function expectResetToInitialView(page: Page, opened?: string) {
	await page.waitForTimeout(400);
	const initial = opened ?? (await viewOf(page));
	await page.locator('.svelte-flow__controls-zoomin').first().click();
	await expect.poll(() => viewOf(page)).not.toBe(initial);
	await page.getByRole('button', { name: 'Reset' }).click();
	await expect.poll(() => viewOf(page), { message: 'the view after Reset' }).toBe(initial);
}

async function checkLayout(page: Page, label: string) {
	// `settle` can return before a graph has mounted (no canvas yet counts as settled), so a graph may
	// still be mid-fit here on a busy machine. Give it time; one that never fits still fails below.
	await page.waitForFunction(() => !document.querySelector('.graph-canvas.unfitted'), null, { timeout: 30_000 }).catch(() => {});
	const found = await page.evaluate((maxRows) => {
		const problems: string[] = [];
		const width = window.innerWidth;
		// The page itself never scrolls sideways; lists scroll inside their own box.
		if (document.documentElement.scrollWidth > width + 1) problems.push(`page is ${document.documentElement.scrollWidth}px wide in a ${width}px window`);
		// Page headers and toolbars stay inside the window.
		for (const el of document.querySelectorAll<HTMLElement>('.app-topbar, .list-toolbar, .head, .toolbar, article > header, .page-body-inner > header')) {
			// Cards on a graph's canvas pan out of view by design; the canvas clips them.
			if (el.closest('.svelte-flow')) continue;
			const box = el.getBoundingClientRect();
			if (box.width && box.right > width + 1) problems.push(`${el.className.split(' ')[0]} ends at ${Math.round(box.right)}px`);
			for (const child of el.querySelectorAll<HTMLElement>('*')) {
				const c = child.getBoundingClientRect();
				if (c.width && c.right > width + 1 && getComputedStyle(child).visibility !== 'hidden') {
					problems.push(`${el.className.split(' ')[0]}: "${(child.textContent ?? '').trim().slice(0, 40)}" ends at ${Math.round(c.right)}px`);
					break;
				}
			}
		}
		// Every graph has finished its first fit: an unfitted graph stays hidden.
		if (document.querySelector('.graph-canvas.unfitted')) problems.push('a graph never fitted');
		// A graph opens with its card titles readable: compact titles at least 11 px on screen, full cards
		// only down to the compact zoom, 0.82, where their 12.5 px titles show at 10.25 px. (A graph with the
		// library's own controls is drawn whole at any zoom, as Lineage's very large graphs are.)
		for (const canvas of document.querySelectorAll<HTMLElement>('.graph-canvas:not(.unfitted)')) {
			if (!canvas.querySelector('.fit-control')) continue;
			const scale = /scale\(([\d.]+)\)/.exec(canvas.querySelector<HTMLElement>('.svelte-flow__viewport')?.style.transform ?? '');
			const compact = canvas.classList.contains('compact');
			const title = [...canvas.querySelectorAll<HTMLElement>(compact ? '.svelte-flow__node .cface .ctitle' : '.svelte-flow__node .name')].find((t) => t.offsetParent !== null);
			if (!scale || !title) continue;
			const px = parseFloat(getComputedStyle(title).fontSize) * Number(scale[1]);
			if (px < (compact ? 10.95 : 10.2)) problems.push(`${compact ? 'compact' : 'card'} titles show at ${px.toFixed(1)} px`);
		}
		// Every scroll area has the themed scrollbar, not the browser's default.
		for (const el of [document.documentElement, ...document.querySelectorAll<HTMLElement>('body *')]) {
			const style = getComputedStyle(el);
			const scrolls = (/(auto|scroll)/.test(style.overflowY) && el.scrollHeight > el.clientHeight + 1) || (/(auto|scroll)/.test(style.overflowX) && el.scrollWidth > el.clientWidth + 1);
			if (scrolls && style.scrollbarColor === 'auto') {
				problems.push(`${el.tagName.toLowerCase()}.${String(el.className).split(' ')[0]} scrolls with the default scrollbar`);
				break;
			}
		}
		for (const table of document.querySelectorAll<HTMLTableElement>('table.sb-table')) {
			const rows = table.tBodies[0]?.rows ?? [];
			if (rows.length > maxRows) problems.push(`a list draws ${rows.length} rows`);
			// Header cells line up with the first full row's cells.
			const heads = [...(table.tHead?.rows[0]?.cells ?? [])];
			const body = [...rows].find((r) => !r.classList.contains('spacer') && !r.classList.contains('load-row') && r.cells.length === heads.length);
			if (body) {
				heads.forEach((th, i) => {
					const [h, d] = [th.getBoundingClientRect(), body.cells[i].getBoundingClientRect()];
					if (Math.abs(h.left - d.left) > 1 || Math.abs(h.width - d.width) > 1) problems.push(`header "${th.textContent?.trim()}" is off its column by ${Math.round(h.left - d.left)}px`);
				});
			}
		}
		return problems;
	}, MAX_ROWS);
	found.push(...(await page.evaluate(statusLines)));
	// A graph larger than its view opens with the card it starts on wholly inside the canvas.
	found.push(...((await page.evaluate(`(${anchorOutside.toString()})()`)) as string[]));
	expect(found, label).toEqual([]);
}

/**
 * Scrolls each list with a sticky header and checks that just above its header is the header or the
 * top bar, never a row; then the same for sticky headers in each page body, such as the overview's
 * timeline header, which must never show the page's content through a gap above it.
 */
async function checkStickyHeaders(page: Page, label: string) {
	const found = await page.evaluate(async () => {
		const problems: string[] = [];
		const frame = () => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
		for (const body of document.querySelectorAll<HTMLElement>('.page-body')) {
			if (body.scrollHeight <= body.clientHeight) continue;
			const sticky = [...body.querySelectorAll<HTMLElement>('*')].filter((el) => getComputedStyle(el).position === 'sticky');
			if (!sticky.length) continue;
			// Room below the page, so a header near its end can still scroll up to the top and stick.
			const spacer = document.createElement('div');
			spacer.style.height = `${body.clientHeight}px`;
			body.firstElementChild?.appendChild(spacer);
			for (const el of sticky) {
				// Scroll the header's own section up under it, so the header is stuck at the top of the body
				// with its section's content passing beneath; a header whose section is gone isn't stuck.
				body.scrollTop = 0;
				await frame();
				const section = el.parentElement!.getBoundingClientRect();
				const natural = el.getBoundingClientRect().top - body.getBoundingClientRect().top;
				const room = section.height - el.getBoundingClientRect().height - 4;
				if (room < 8) continue;
				body.scrollTop = natural + Math.min(120, room);
				await frame();
				const box = el.getBoundingClientRect();
				const top = body.getBoundingClientRect().top + body.clientTop;
				if (!box.width || !box.height) continue;
				// Not stuck (the body couldn't scroll that far, or it sticks in a scroller of its own): nothing to check.
				const resting = top + natural - body.scrollTop;
				if (box.top <= resting + 1) continue;
				// Every point between the body's top edge and the stuck header must be the header itself or
				// another sticky header: anything else is content showing through a gap above it.
				for (let y = top; y < box.top; y += 2) {
					const hit = document.elementFromPoint(box.left + Math.min(box.width / 2, 200), y);
					if (hit && body.contains(hit) && !sticky.some((s) => s.contains(hit))) {
						problems.push(`page content shows ${Math.round(box.top - top)}px above a sticky ${el.tagName.toLowerCase()}.${String(el.className).split(' ')[0]}`);
						break;
					}
				}
			}
			spacer.remove();
			body.scrollTop = 0;
		}
		for (const scroller of document.querySelectorAll<HTMLElement>('.sb-table-scroll:not(.embedded)')) {
			if (scroller.scrollHeight <= scroller.clientHeight) continue;
			scroller.scrollTop = 2000;
			await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
			const head = scroller.querySelector('thead');
			if (!head) continue;
			const box = head.getBoundingClientRect();
			const x = box.left + Math.min(box.width / 2, 200);
			for (const y of [box.top - 1, box.top + 1]) {
				const hit = document.elementFromPoint(x, y);
				if (hit?.closest('tbody')) problems.push(`a row shows at y=${Math.round(y)}, around the sticky header`);
			}
		}
		return problems;
	});
	expect(found, label).toEqual([]);
}

for (const width of WIDTHS) {
	test.describe(`${width}px`, () => {
		test.use({ viewport: { width, height: 1000 } });

		for (const path of PAGES) {
			test(`${path} fits, lines up and windows its lists`, async ({ page }) => {
				await page.goto(awkward(path));
				await settle(page);
				await checkLayout(page, path);
				await checkStickyHeaders(page, path);
			});
		}

		// The two long lists fit their box at desktop widths: no sideways scrollbar for a few pixels of columns.
		for (const path of ['/catalog', '/checks']) {
			test(`${path}'s list does not scroll sideways`, async ({ page }) => {
				await page.goto(awkward(path));
				await settle(page);
				const box = page.locator('.sb-table-scroll').first();
				const [scroll, client] = await box.evaluate((el) => [el.scrollWidth, el.clientWidth]);
				expect(scroll, `${path}: list ${scroll}px wide in a ${client}px box`).toBeLessThanOrEqual(client + 1);
			});
		}

		// Cards drawn in full, so their status lines show: a job's node cards, and a group's folder and
		// job cards. Zoomed in until cards aren't compact, as a narrow window may open them compact.
		for (const path of ['/jobs/core_build', '/jobs/warehouse_nightly']) {
			test(`${path}'s cards keep their status line on one line`, async ({ page }) => {
				await page.goto(awkward(path));
				await settle(page);
				const canvas = page.locator('.graph-canvas').first();
				const pane = (await page.locator('.svelte-flow').first().boundingBox())!;
				await page.mouse.move(pane.x + pane.width / 2, pane.y + pane.height / 2);
				for (let i = 0; i < 20 && /\bcompact\b/.test((await canvas.getAttribute('class')) ?? ''); i++) {
					await page.mouse.wheel(0, -60);
					await page.waitForTimeout(300);
				}
				await expect(canvas).not.toHaveClass(/\bcompact\b/);
				expect(await page.locator('.svelte-flow__node .status').count(), 'status lines').toBeGreaterThan(3);
				expect(await page.evaluate(statusLines), path).toEqual([]);
			});
		}

		test('a real-sized Lineage overview opens whole, readable, and Reset comes back to it', async ({ page }) => {
			await page.setViewportSize({ width: 1900, height: 1060 });
			await page.goto('/lineage?mock&large');
			await settle(page);
			await checkLayout(page, 'large Lineage');
			// About 13 folder cards: every one wholly in view, none cut by the canvas edge.
			const cut = await page.evaluate(() => {
				const flow = document.querySelector<HTMLElement>('.svelte-flow')!;
				const pane = flow.getBoundingClientRect();
				const cards = [...flow.querySelectorAll<HTMLElement>('.svelte-flow__node')];
				return { cards: cards.length, cut: cards.filter((n) => { const r = n.getBoundingClientRect(); return r.left < pane.left || r.right > pane.right || r.top < pane.top || r.bottom > pane.bottom; }).map((n) => n.dataset.id) };
			});
			expect(cut.cards, 'folder cards drawn').toBeGreaterThanOrEqual(12);
			expect(cut.cut, 'cards outside the canvas').toEqual([]);
			await expectResetToInitialView(page);
		});

		test('the large mock group draws its direct waits only and opens on its cards', async ({ page }) => {
			await page.goto(awkward(`/jobs/${BIG_GROUP}`));
			await settle(page);
			await checkLayout(page, BIG_GROUP);
			const drawn = () =>
				page.evaluate(() => {
					const pane = document.querySelector('.svelte-flow')!.getBoundingClientRect();
					const rect = (s: string) => [...document.querySelectorAll<HTMLElement>(s)].map((n) => n.getBoundingClientRect());
					const cards = rect('.svelte-flow__node[data-id^="member:"], .svelte-flow__node[data-id^="jobfolder:"]');
					const inside = cards.filter((c) => c.width && c.left >= pane.left && c.right <= pane.right && c.top >= pane.top && c.bottom <= pane.bottom);
					return { members: rect('.svelte-flow__node[data-id^="member:"]').length, folders: rect('.svelte-flow__node[data-id^="jobfolder:"]').length, cards: cards.length, edges: document.querySelectorAll('.svelte-flow__edge').length, inside: inside.length };
				});
			const opened = await viewOf(page);
			// A group this large opens by folder, closed, with a line per pair of folders.
			let found = await drawn();
			expect(found.folders, 'closed folders drawn').toBeGreaterThan(1);
			expect(found.edges, 'lines between folders').toBeLessThanOrEqual(2 * found.cards);
			expect(found.inside, 'cards wholly in view after load').toBeGreaterThan(0);
			// Without folders: every member, and only the waits no other wait implies, at most two lines per member.
			await page.getByRole('button', { name: 'Folders' }).click();
			await expect.poll(async () => (await drawn()).members).toBeGreaterThanOrEqual(40);
			found = await drawn();
			expect(found.edges, 'lines drawn').toBeLessThanOrEqual(2 * found.members);
			// Larger than the view now: the refit starts on a card wholly in view.
			await page.waitForTimeout(500);
			expect(await page.evaluate(`(${anchorOutside.toString()})()`), 'start card after Folders off').toEqual([]);
			expect(await page.locator('.svelte-flow[data-fit-anchor]').getAttribute('data-fit-anchor')).toMatch(/^member:/);
			// The shared toolbar: Expand all opens members in place, Reset closes them and brings folders back.
			const boxes = () => page.locator('.svelte-flow__node-memberBox').count();
			await page.getByRole('button', { name: 'Expand all' }).click();
			await expect.poll(boxes).toBeGreaterThan(0);
			await page.getByRole('button', { name: 'Reset' }).click();
			await expect.poll(boxes).toBe(0);
			await expect.poll(async () => (await drawn()).folders).toBeGreaterThan(1);
			// Back to the view it opened on, after all that, and again after zooming in.
			await expect.poll(() => viewOf(page), { message: 'the view after Reset' }).toBe(opened);
			await expectResetToInitialView(page, opened);
		});

		for (const { list, link } of ITEMS) {
			test(`the first item of ${list} fits and lines up`, async ({ page }) => {
				await page.goto(awkward(list));
				await settle(page);
				const href = await page.locator(`table ${link}`).first().getAttribute('href');
				test.skip(!href, `${list} has no item to open`);
				await page.goto(awkward(href!));
				await settle(page);
				await checkLayout(page, href!);
				await checkStickyHeaders(page, href!);
			});
		}
	});
}
