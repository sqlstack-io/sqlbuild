import { expect, test, type Page } from '@playwright/test';

// The plan page's list as an index and a pane, on a plan the size of a real `build --changed`
// (`?mock&large`): one section at a time, large sections' rows a page at a time. The mock makes no
// network requests, so the plan rows reads are counted from its own record (`window.__sqbPlanRows`).
// With SHOTS set, each section is photographed: SHOTS=/some/dir npx playwright test e2e/plan-browser.spec.ts
const shots = process.env.SHOTS;
test.use({ viewport: { width: 1600, height: 1000 } });

const LARGE = `/plan?mock&large&cmd=${encodeURIComponent('sqb build --changed')}`;
type Call = { section: string; offset: number; limit: number };
const calls = (page: Page) => page.evaluate(() => (window as unknown as { __sqbPlanRows?: Call[] }).__sqbPlanRows ?? []);
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
	expect(await calls(page), 'no rows read before a paged section is opened').toEqual([]);
	await select(page, 'unchanged');
	await expect(rows(page)).toHaveCount(50);
	expect(await calls(page)).toEqual([{ section: 'routine_models', offset: 0, limit: 50 }]);
	// Every model of it runs for the same reason: said once, not on each row.
	await expect(pane(page).locator('.subtitle')).toContainText('models: no completed record');
	await expect(rows(page).locator('.detail', { hasText: 'no completed record' })).toHaveCount(0);
	if (shots) await page.screenshot({ path: `${shots}/plan-b-unchanged.png` });
	await page.getByRole('button', { name: /^Show more/ }).click();
	await expect(rows(page)).toHaveCount(100);
	expect(await calls(page)).toEqual([
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

	const big = rows(page).filter({ hasText: 'query +71 −50' }).first();
	await big.getByRole('button', { name: 'diff' }).click();
	const diff = big.locator('pre.diff');
	await expect(diff).toBeVisible();
	expect(await diff.locator('span.add, span.del').count()).toBe(12);
	await big.getByRole('button', { name: 'open the full diff' }).click();
	expect(await diff.locator('span.add, span.del').count()).toBe(121);
	if (shots) await big.scrollIntoViewIfNeeded().then(() => page.screenshot({ path: `${shots}/plan-b-changed-diff.png` }));
	await big.getByRole('button', { name: 'hide diff' }).click();
	await expect(diff).toHaveCount(0);
	// A config change of more than two keys opens as a diff too.
	await expect(rows(page).locator('.detail', { hasText: /^config · diff$/ })).toHaveCount(1);
});

test('the filter narrows the section to matching rows', async ({ page }) => {
	await openPlan(page);
	await select(page, 'python');
	await pane(page).getByLabel('Filter python').fill('orders_export_12');
	await expect(rows(page)).toHaveCount(2);
	for (const name of await rows(page).locator('.name').allInnerTexts()) expect(name).toContain('orders_export_12');
});

test('warnings with one message about different names read as one row, opened to the list', async ({ page }) => {
	await openPlan(page);
	await select(page, 'warnings');
	const groups = pane(page).locator('.warning-group');
	await expect(groups).toHaveCount(2);
	await expect(groups.first()).toContainText('Scenario … provides fixture for unknown model ….');
	await expect(groups.first()).toContainText('71 in 24 scenarios');
	await groups.first().getByRole('button', { name: 'show' }).click();
	await expect(groups.first().locator('li')).toHaveCount(71);
	if (shots) await page.screenshot({ path: `${shots}/plan-b-warnings.png` });
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
