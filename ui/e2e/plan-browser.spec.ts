import { expect, test, type Page } from '@playwright/test';

// The plan page's list as an index and a pane, on a plan the size of a real `build --changed`
// (`?mock&large`): one section at a time, large sections' rows a page at a time. The mock makes no
// network requests, so the plan rows and diff reads are counted from its own record
// (`window.__sqbPlanRows`, `window.__sqbPlanDiffs`).
// With SHOTS set, each section is photographed: SHOTS=/some/dir npx playwright test e2e/plan-browser.spec.ts
const shots = process.env.SHOTS;
test.use({ viewport: { width: 1600, height: 1000 } });

const LARGE = `/plan?mock&large&cmd=${encodeURIComponent('sqb build --changed')}`;
type Call = { section: string; offset: number; limit: number; name?: string };
const calls = (page: Page) => page.evaluate(() => (window as unknown as { __sqbPlanRows?: Call[] }).__sqbPlanRows ?? []);
const diffReads = (page: Page) => page.evaluate(() => (window as unknown as { __sqbPlanDiffs?: string[] }).__sqbPlanDiffs ?? []);
/** The large mock's model whose diff the server withholds, as without permission to read its SQL. */
const WITHHELD = 'suppliers__stg__snapshot_79';
const pane = (page: Page) => page.locator('.pane');
const rows = (page: Page) => page.locator('.pane .plan-row');

async function openPlan(page: Page, path = LARGE) {
	await page.goto(path);
	await expect(page.locator('.plan-browser')).toBeVisible({ timeout: 30_000 });
}
async function select(page: Page, key: string) {
	await page.locator(`.index .entry[data-section="${key}"]`).click();
	await expect(pane(page)).toHaveAttribute('data-section', key);
}

test('the index lists the sections in order, opens on the first, and the address keeps the one picked', async ({ page }) => {
	await openPlan(page);
	await expect(page).toHaveTitle('Plan · sqb build --changed');
	await expect(page.locator('.index .summary')).toHaveText('94 changed, 672 new, 121 renames, 64 loads, 433 Python, 40 held back · 1,519 unchanged, 300 skipped');
	expect(await page.locator('.index .entry').evaluateAll((es) => es.map((e) => (e as HTMLElement).dataset.section))).toEqual(['changed', 'renames', 'first_run', 'loads', 'python', 'held_back', 'warnings', 'unchanged', 'skipped']);
	await expect(page.locator('.index .entry[aria-current="true"]')).toHaveAttribute('data-section', 'changed');
	await select(page, 'first_run');
	await expect(page).toHaveURL(/[?&]section=first_run/);
	await expect(pane(page).locator('h3')).toContainText('First run');
	// A first run's range is its own bounds, in words.
	await expect(pane(page).locator('.note', { hasText: 'from the last run → the latest data when the run starts' }).first()).toBeVisible();
	await expect(pane(page)).not.toContainText('__SQB_CURSOR');
	await page.goto(`${LARGE}&section=python`);
	await expect(pane(page)).toHaveAttribute('data-section', 'python', { timeout: 30_000 });
	await expect(page.locator('.index .entry[aria-current="true"]')).toHaveAttribute('data-section', 'python');
});

test('a large section loads only its first page when opened, and Show more loads the next', async ({ page }) => {
	await openPlan(page);
	await expect(rows(page)).toHaveCount(50);
	expect(await calls(page), 'the section opened reads its first page, and no other').toEqual([{ section: 'query_changed', offset: 0, limit: 50 }]);
	await select(page, 'unchanged');
	await expect(rows(page)).toHaveCount(50);
	expect((await calls(page)).slice(1)).toEqual([{ section: 'routine_models', offset: 0, limit: 50 }]);
	// Every model of it runs for the same reason, by the server's count: said once, not on each row.
	await expect(pane(page).locator('.subtitle')).toContainText('models: no completed record (all 1,507)');
	await expect(rows(page).locator('.detail', { hasText: 'no completed record' })).toHaveCount(0);
	if (shots) await page.screenshot({ path: `${shots}/plan-b-unchanged.png` });
	await page.getByRole('button', { name: /^Show more/ }).click();
	await expect(rows(page)).toHaveCount(100);
	expect((await calls(page)).slice(1)).toEqual([
		{ section: 'routine_models', offset: 0, limit: 50 },
		{ section: 'routine_models', offset: 50, limit: 50 }
	]);
});

test('a changed row shows its kind and line counts, and opens its diff a few lines at a time', async ({ page }) => {
	await openPlan(page);
	await expect(pane(page).locator('.subtitle')).toHaveText('87 query · 6 config · 1 function');
	// Whitespace-only diffs are labelled and lose their counts; a change of keyword case is a change.
	await expect(rows(page).locator('.detail', { hasText: /^whitespace only/ })).toHaveCount(0);
	await page.getByRole('button', { name: /^Show more/ }).click();
	await expect(rows(page)).toHaveCount(94);
	await expect(rows(page).locator('.detail', { hasText: /^whitespace only/ })).toHaveCount(5);
	await expect(rows(page).locator('.detail', { hasText: /^query \+1 −1/ })).toHaveCount(2);
	await expect(rows(page).locator('.detail', { hasText: 'config · full_refresh true → false' })).toHaveCount(4);
	await expect(rows(page).locator('.detail', { hasText: 'config + query · cursor observed_at → event_date' })).toHaveCount(1);
	if (shots) await page.screenshot({ path: `${shots}/plan-b-changed.png` });
	// Rows carry counts, not diff text: only the small config changes shown inline have been read.
	expect(await diffReads(page)).toHaveLength(5);

	const big = rows(page).filter({ hasText: 'query +71 −50' }).first();
	const bigName = await big.locator('.name').innerText();
	await big.getByRole('button', { name: 'diff' }).click();
	const diff = big.locator('pre.diff');
	await expect(diff).toBeVisible();
	expect((await diffReads(page)).filter((m) => m === bigName)).toHaveLength(1);
	// Opened: the headers, a little context and the first 12 changed lines, then a link to the rest.
	expect(await diff.locator('span.add, span.del').count()).toBe(12);
	expect(await diff.locator('span').count()).toBeLessThanOrEqual(16);
	const full = big.getByRole('button', { name: /^open the full diff \(\d+ more lines\)$/ });
	await expect(full).toHaveText('open the full diff (110 more lines)');
	if (shots) await big.scrollIntoViewIfNeeded().then(() => page.screenshot({ path: `${shots}/plan-b-changed-diff.png` }));
	await full.click();
	expect(await diff.locator('span.add, span.del').count()).toBe(121);
	expect(await diff.locator('span').count()).toBe(126);
	await expect(full).toHaveCount(0);
	// Closing returns the row to closed; opening it again starts from the short view.
	await big.getByRole('button', { name: 'hide diff' }).click();
	await expect(diff).toHaveCount(0);
	await big.getByRole('button', { name: 'diff' }).click();
	await expect(diff).toBeVisible();
	expect(await diff.locator('span.add, span.del').count()).toBe(12);
	await big.getByRole('button', { name: 'hide diff' }).click();
	expect((await diffReads(page)).filter((m) => m === bigName), 'a diff read once is kept').toHaveLength(1);
	// A config change of more than two keys opens as a diff too.
	await expect(rows(page).locator('.detail', { hasText: /^config · diff$/ })).toHaveCount(1);
	// A diff the server withholds says so, quietly.
	const withheld = rows(page).filter({ has: page.locator('.name', { hasText: WITHHELD }) });
	await withheld.getByRole('button', { name: 'diff' }).click();
	await expect(withheld.locator('.diff-note')).toHaveText('diff not available');
	await expect(withheld.locator('pre.diff')).toHaveCount(0);
});

test('the filter asks the server for matching rows in the whole section, not just those loaded', async ({ page }) => {
	await openPlan(page);
	await select(page, 'python');
	await pane(page).getByLabel('Filter python').fill('orders_export_12');
	await expect(rows(page)).toHaveCount(2);
	for (const name of await rows(page).locator('.name').allInnerTexts()) expect(name).toContain('orders_export_12');
	expect((await calls(page)).filter((c) => c.name)).toEqual([{ section: 'python_nodes', offset: 0, limit: 50, name: 'orders_export_12' }]);
	// A large paged section: matches from beyond the first page, a page at a time.
	await select(page, 'unchanged');
	await pane(page).getByLabel('Filter unchanged').fill('RETURNS__');
	await expect(rows(page)).toHaveCount(50);
	await expect(pane(page)).not.toContainText('Filtering the');
	for (const name of await rows(page).locator('.name').allInnerTexts()) expect(name).toContain('returns__');
	const more = page.getByRole('button', { name: /^Show more · \d+ not shown$/ });
	await expect(more).toBeVisible();
	await more.click();
	await expect(rows(page)).toHaveCount(100);
	expect((await calls(page)).filter((c) => c.name === 'RETURNS__').map((c) => [c.section, c.offset])).toEqual([
		['routine_models', 0],
		['functions', 0],
		['routine_models', 50]
	]);
	await pane(page).getByLabel('Filter unchanged').fill('no such name');
	await expect(pane(page).locator('.hint')).toHaveText('No unchanged match “no such name”.');
});

test('warnings with one message about different names read as one row, opened to the list', async ({ page }) => {
	await openPlan(page);
	await select(page, 'warnings');
	const groups = pane(page).locator('.warning-group');
	await expect(groups).toHaveCount(2);
	// Warnings group by message once read, so the pane reads them all (up to a cap) in large pages.
	expect((await calls(page)).filter((c) => c.section === 'warnings')).toEqual([{ section: 'warnings', offset: 0, limit: 200 }]);
	await expect(groups.first().locator('.template')).toHaveText('Scenario … provides fixture for unknown model …');
	await expect(groups.first()).toContainText('71 in 24 scenarios');
	await groups.first().getByRole('button', { name: 'show', exact: true }).click();
	await expect(groups.first().locator('li')).toHaveCount(20);
	if (shots) await page.screenshot({ path: `${shots}/plan-b-warnings.png` });
	await groups.first().getByRole('button', { name: 'show all 71' }).click();
	await expect(groups.first().locator('li')).toHaveCount(71);
	await expect(groups.first().getByRole('button', { name: 'show all 71' })).toHaveCount(0);
});

test('a small plan reads as a short index', async ({ page }) => {
	await openPlan(page, `/plan?mock&cmd=${encodeURIComponent('sqb build -s path:models/marts')}`);
	const entries = page.locator('.index .entry');
	expect(await entries.count()).toBeGreaterThanOrEqual(1);
	expect(await entries.count()).toBeLessThanOrEqual(3);
	if (shots) await page.screenshot({ path: `${shots}/plan-b-small.png` });
});

test('photographs every section of the large plan', async ({ page }) => {
	test.skip(!shots, 'SHOTS is not set');
	await openPlan(page);
	for (const key of ['changed', 'renames', 'first_run', 'loads', 'python', 'held_back', 'skipped']) {
		await select(page, key);
		await expect(rows(page).first()).toBeVisible();
		await page.screenshot({ path: `${shots}/plan-b-${key}.png` });
	}
});
