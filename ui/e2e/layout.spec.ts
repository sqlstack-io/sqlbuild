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

const awkward = (path: string) => `${path}${path.includes('?') ? '&' : '?'}mock&awkward`;

async function settle(page: Page) {
	await page.waitForLoadState('networkidle');
	await page.waitForTimeout(500);
}

async function checkLayout(page: Page, label: string) {
	const found = await page.evaluate((maxRows) => {
		const problems: string[] = [];
		const width = window.innerWidth;
		// The page itself never scrolls sideways; lists scroll inside their own box.
		if (document.documentElement.scrollWidth > width + 1) problems.push(`page is ${document.documentElement.scrollWidth}px wide in a ${width}px window`);
		// Page headers and toolbars stay inside the window.
		for (const el of document.querySelectorAll<HTMLElement>('.app-topbar, .list-toolbar, .head, .toolbar')) {
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
	expect(found, label).toEqual([]);
}

/** Scrolls each list with a sticky header and checks that just above its header is the header or the top bar, never a row. */
async function checkStickyHeaders(page: Page, label: string) {
	const found = await page.evaluate(async () => {
		const problems: string[] = [];
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
