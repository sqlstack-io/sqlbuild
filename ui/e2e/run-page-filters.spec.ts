import { expect, test, type Page } from '@playwright/test';

// Every control on the run page that filters toggles: the first click turns its filter on and says so,
// the second puts the page back exactly as it was. Each text field clears with its ✕ or Esc, keeping focus.
test.use({ viewport: { width: 1500, height: 1000 } });

const RUN = '/runs/run-pa93?mock&failure-mock';
const summary = (page: Page) => page.getByTestId('run-summary');
const logCount = (page: Page) => page.getByRole('tab', { name: /^Log/ }).textContent();
const logFilter = (page: Page) => page.getByLabel('Filter the log');
const selector = (page: Page) => page.getByLabel('Selection', { exact: true });

async function open(page: Page, url = RUN) {
	await page.goto(url);
	await expect(summary(page)).toContainText('built', { timeout: 20_000 });
}

/** What the log shows and how the page reads: rows, the Showing line, what is pressed, and the layout around it. */
async function snapshot(page: Page) {
	await page.waitForTimeout(300);
	return {
		box: (await logFilter(page).inputValue()).trim(),
		line: (await page.getByTestId('log-filter-active').count()) ? (await page.getByTestId('log-filter-active').textContent())?.trim() : null,
		rows: await page.locator('.log tbody tr').evaluateAll((rows) => rows.slice(0, 40).map((r) => (r.textContent ?? '').replace(/\s+/g, ' ').trim())),
		pressed: await page.locator('[aria-pressed="true"], [aria-current="true"]').evaluateAll((els) => els.map((e) => (e.textContent ?? '').trim()).filter(Boolean).sort()),
		layout: [await logCount(page), await page.getByRole('radio', { name: 'Events' }).count(), await page.getByRole('button', { name: 'Levels' }).count(), await page.getByRole('button', { name: 'Kinds' }).count() + (await page.locator('.filters .menu-trigger').count())]
	};
}

// Every quick filter only writes words into the log's filter box: clicking it shows exactly what typing
// those words does, the layout never moves, and a second click takes out only its own words.
const QUICK_FILTERS: { run: string; name: string; control: (page: Page) => ReturnType<Page['getByRole']>; text: string }[] = [
	{ run: RUN, name: 'the failed-audits figure', control: (p) => summary(p).getByRole('link', { name: '1 audit failed' }), text: 'kind:audit outcome:failed' },
	{ run: RUN, name: 'the built figure', control: (p) => summary(p).getByRole('link', { name: '93 built' }), text: 'outcome:built' },
	{ run: '/runs/run-4f2a?mock', name: 'the skipped figure', control: (p) => summary(p).getByRole('link', { name: /^\d+ skipped$/ }), text: 'outcome:skipped' },
	{ run: RUN, name: 'the Errors chip', control: (p) => p.getByRole('button', { name: /^Errors \d/ }), text: 'level:error' }
];

for (const q of QUICK_FILTERS) {
	test(`${q.name} does exactly what typing ${q.text} does`, async ({ page }) => {
		await open(page, q.run);
		const before = await snapshot(page);
		await q.control(page).click();
		const clicked = await snapshot(page);
		expect(clicked.box).toBe(q.text);
		expect(clicked.line).toMatch(/^Showing .+ · [\d,]+ of [\d,]+ events?$/);
		expect(clicked.layout).toEqual(before.layout);
		await q.control(page).click();
		expect(await snapshot(page)).toEqual(before);
		await logFilter(page).fill(`${q.text} `);
		await logFilter(page).evaluate((el) => (el as HTMLInputElement).blur());
		expect(await snapshot(page)).toEqual(clicked);
	});

	test(`${q.name} moves nothing in the log toolbar`, async ({ page }) => {
		await open(page, q.run);
		// The toolbar, the filter box, each control in it, and the table's header under the Showing line.
		const boxes = () =>
			page.evaluate(() =>
				['.log-head .filters', '.log-filter .field', '.filters .menu-trigger', '.filters .count', '.log thead'].flatMap((sel) =>
					[...document.querySelectorAll(sel)].map((el) => {
						const r = el.getBoundingClientRect();
						return `${sel} ${r.x},${r.y},${r.width},${r.height}`;
					})
				)
			);
		const before = await boxes();
		expect(before.length).toBeGreaterThan(3);
		await q.control(page).click();
		await expect(page.getByTestId('log-filter-active')).toBeVisible();
		expect(await boxes()).toEqual(before);
		await q.control(page).click();
		await expect(page.getByTestId('log-filter-active')).toHaveCount(0);
		expect(await boxes()).toEqual(before);
	});

	test(`${q.name}'s second click keeps what was typed`, async ({ page }) => {
		await open(page, q.run);
		await logFilter(page).fill('orders');
		await q.control(page).click();
		await expect(logFilter(page)).toHaveValue(`orders ${q.text} `);
		await expect(page.getByTestId('log-filter-active')).toContainText(`Showing orders ${q.text}`);
		await q.control(page).click();
		await expect(logFilter(page)).toHaveValue(/^orders\s*$/);
	});
}

for (const how of ['✕', 'Esc'] as const) {
	test(`the node selector clears with ${how} and keeps focus`, async ({ page }) => {
		await open(page);
		await selector(page).fill('currency');
		const clear = page.getByRole('button', { name: 'Clear selection' });
		await expect(clear).toBeVisible();
		if (how === '✕') await clear.click();
		else await selector(page).press('Escape');
		await expect(selector(page)).toHaveValue('');
		await expect(clear).toHaveCount(0);
		await expect(selector(page)).toBeFocused();
	});

	test(`the log filter clears with ${how} and keeps focus`, async ({ page }) => {
		await open(page);
		await logFilter(page).fill('level:error');
		const clear = page.getByRole('button', { name: 'Clear the log filter' });
		await expect(clear).toBeVisible();
		if (how === '✕') await clear.click();
		else await logFilter(page).press('Escape');
		await expect(logFilter(page)).toHaveValue('');
		await expect(clear).toHaveCount(0);
		await expect(logFilter(page)).toBeFocused();
	});
}

// The rerun plan holds no state of its own: it opens on failed:<run>+, the box's own suggestions offer the
// run selectors, and "from run" is read from the selection.
test('the rerun plan opens on failed:<run>+ and the box suggests the run selectors', async ({ page }) => {
	await page.goto('/plan/rerun?run=run-pa93&mode=from_failure&mock&failure-mock');
	const box = page.locator('.sel-field').first();
	await expect(box).toContainText('failed:run-pa93+', { timeout: 20_000 });
	await expect(page.getByTestId('rerun-of')).toHaveCount(0);
	await expect(page.getByRole('listbox')).toHaveCount(0);
	await expect(page.getByTestId('rerun-from')).toContainText('from run');
	await expect(page.getByRole('cell', { name: 'orders_match_payments' })).toBeVisible();
	await page.getByRole('button', { name: 'Clear selection' }).click();
	await selector(page).fill('run:');
	const whole = page.getByRole('option', { name: /run:run-pa93\s*the whole run/ });
	await expect(whole).toBeVisible();
	await whole.click();
	await expect(box).toContainText('run:run-pa93');
	await expect(page.getByTestId('rerun-from')).toContainText('from run');
	await page.getByRole('button', { name: 'Clear selection' }).click();
	await selector(page).fill('country_codes ');
	await expect(page.getByTestId('rerun-from')).toHaveCount(0);
});
