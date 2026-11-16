import { expect, test, type Page } from '@playwright/test';

// The toolbar's choice filter with thousands of options: the Checks page with awkward mock data has
// 5,000 checks on about 2,000 long-named nodes. Opening Node shows a search box, draws a bounded
// number of rows, keeps every label inside its row and every count inside the panel, and ticking two
// options narrows the list to them. Set CHOICE_FILTER_SHOT to a path to save a screenshot.

test.use({ viewport: { width: 1440, height: 1000 } });

const FRAGMENT = 'enough_to_show_17';

/**
 * The list's drawn data rows: not the spacers, and not the loading row shown until the checks arrive.
 * The mock answers after a delay, which `networkidle` doesn't wait for (it makes no requests), so tests
 * wait for these rows rather than for the network.
 */
const DATA_ROWS = 'table.sb-table tbody tr:not(.spacer):not(.load-row)';

/** Opens Checks, `query` added to the URL, once its checks are drawn. */
async function openChecks(page: Page, query = '') {
	await page.goto(`/checks?mock&awkward${query}`);
	// Thousands of awkward checks can take a while to arrive on a busy machine.
	await expect(page.locator(DATA_ROWS).first()).toBeVisible({ timeout: 20_000 });
}

/** The "Attached to" cell's full text (`node` or `node.column`) of each drawn row. */
const nodeCells = (page: Page) => page.locator(`${DATA_ROWS} td:nth-child(3)`).evaluateAll((cells) => cells.map((c) => (c as HTMLElement).title));
const onNode = (cell: string, node: string) => cell === node || cell.startsWith(`${node}.`);

/** Problems with the open panel's rows: labels spilling out of their row, counts outside the panel. */
async function panelProblems(page: Page) {
	return page.evaluate(() => {
		const problems: string[] = [];
		const panel = document.querySelector<HTMLElement>('.choice-panel');
		if (!panel) return ['no open panel'];
		const box = panel.getBoundingClientRect();
		if (box.right > window.innerWidth - 4) problems.push(`panel ends at ${Math.round(box.right)}px`);
		for (const row of panel.querySelectorAll<HTMLElement>('.cf-row')) {
			const r = row.getBoundingClientRect();
			const label = row.querySelector<HTMLElement>('.cf-label')!.getBoundingClientRect();
			const count = row.querySelector<HTMLElement>('.cf-count');
			const name = row.title.slice(-24);
			if (row.scrollWidth > row.clientWidth + 1) problems.push(`row …${name} overflows by ${row.scrollWidth - row.clientWidth}px`);
			if (label.right > r.right + 1) problems.push(`label …${name} ends past its row`);
			if (!count) {
				problems.push(`row …${name} has no count`);
				continue;
			}
			const c = count.getBoundingClientRect();
			if (!c.width || c.left < label.right - 1 || c.right > box.right + 1) problems.push(`count of …${name} is not inside the panel beside its label`);
		}
		return problems;
	});
}

test('a filter with thousands of options searches, windows and takes several values', async ({ page }) => {
	await openChecks(page);
	const trigger = page.getByRole('button', { name: /^Node/ });
	await trigger.click();

	const search = page.getByRole('textbox', { name: 'Search Node' });
	await expect(search).toBeFocused();
	const rows = page.locator('.choice-panel .cf-row');
	await expect(rows.first()).toBeVisible();
	expect(await rows.count(), 'the list is windowed').toBeLessThan(100);
	expect(await panelProblems(page)).toEqual([]);

	await search.fill(FRAGMENT);
	await expect(rows.first()).toBeVisible();
	const titles = await rows.evaluateAll((els) => els.map((e) => (e as HTMLElement).title));
	expect(titles.length).toBeGreaterThan(2);
	for (const t of titles) expect(t.toLowerCase()).toContain(FRAGMENT);
	expect(await panelProblems(page)).toEqual([]);

	await rows.nth(0).click();
	await rows.nth(1).click();
	const picked = titles.slice(0, 2);
	await expect(search, 'focus stays in the search box').toBeFocused();
	await expect(trigger).toContainText('Node:2 selected');
	await expect(rows.nth(0).locator('input')).toBeChecked();

	if (process.env.CHOICE_FILTER_SHOT) {
		// The toolbar and the open filter, with some of the page around them.
		const [a, b] = [await page.locator('.list-toolbar').boundingBox(), await page.locator('.choice-panel').boundingBox()];
		const top = Math.max(0, Math.min(a!.y, b!.y) - 24);
		const bottom = Math.min(1000, Math.max(a!.y + a!.height, b!.y + b!.height) + 24);
		await page.screenshot({ path: process.env.CHOICE_FILTER_SHOT, clip: { x: 0, y: top, width: 1440, height: bottom - top } });
	}

	await page.keyboard.press('Escape');
	await expect(page.locator('.choice-panel')).toHaveCount(0);
	expect(new URL(page.url()).searchParams.getAll('node').sort()).toEqual([...picked].sort());

	const cells = await nodeCells(page);
	expect(cells.length).toBeGreaterThan(0);
	for (const c of cells) expect(picked.some((p) => onNode(c, p)), c).toBe(true);

	await page.getByRole('button', { name: 'Clear Node filter' }).click();
	await expect(trigger).toHaveText(/^\s*Node\s*$/);
});

test('a filter with few options has no search box and works from the keyboard', async ({ page }) => {
	await openChecks(page);
	await page.getByRole('button', { name: /^Attached to/ }).click();
	await expect(page.locator('.choice-panel .cf-row').first()).toBeVisible();
	await expect(page.locator('.choice-panel input[type="search"], .choice-panel .cf-search')).toHaveCount(0);
	await page.keyboard.press('ArrowDown');
	await page.keyboard.press(' ');
	await expect(page.locator('.choice-panel .cf-row').first()).toHaveAttribute('aria-selected', 'true');
	await expect(page.getByRole('button', { name: /^Attached to/ })).toContainText('Attached to:');
});

test('an old single-node link still filters to that node', async ({ page }) => {
	await openChecks(page);
	await page.getByRole('button', { name: /^Node/ }).click();
	const node = await page.locator('.choice-panel .cf-row').nth(3).getAttribute('title');
	await page.keyboard.press('Escape');
	// The button reads the link at once; the rows come with the checks, so wait for those.
	await openChecks(page, `&node=${encodeURIComponent(node!)}`);
	await expect(page.getByRole('button', { name: /^Node/ })).toContainText(`Node:${node!.slice(0, 20)}`);
	const cells = await nodeCells(page);
	expect(cells.length).toBeGreaterThan(0);
	for (const c of cells) expect(onNode(c, node!), c).toBe(true);
});
