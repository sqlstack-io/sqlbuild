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
	await expect(page).toHaveTitle('Plan · sqb build --changed · SQLBuild');
	// Grouped under what will run and what won't, each heading counting nodes once, with no summary sentence.
	await expect(page.locator('.index .summary')).toHaveCount(0);
	await expect(page.getByTestId('index-will-run')).toHaveText('Will run2,770');
	await expect(page.getByTestId('index-wont-run')).toHaveText('Won’t run340');
	expect(await page.locator('.index .entry').evaluateAll((es) => es.map((e) => [(e as HTMLElement).dataset.section, e.querySelector('.count')?.textContent]))).toEqual([
		['changed', '94'],
		// The mock's migrations act on models outside the selection: no renamed nodes, only steps, which
		// the section's header counts.
		['renames', '121 steps'],
		['first_run', '672'],
		['loads', '64'],
		['python', '433'],
		// 1,507 models; the section also lists 12 unchanged functions, which are not nodes.
		['unchanged', '1,507'],
		['held_back', '40'],
		['skipped', '300'],
		['warnings', '74']
	]);
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
	// Rows carry counts and the server's short config change, not diff text: nothing has been read.
	expect(await diffReads(page)).toHaveLength(0);

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
	// The server groups them: the pane lists its groups, counted, and reads a group's warnings when it opens.
	expect((await calls(page)).filter((c) => c.section === 'warnings')).toEqual([]);
	await expect(groups.first().locator('.template')).toHaveText('Scenario … provides fixture for unknown model …');
	await expect(groups.first()).toContainText('71 in 24 scenarios');
	await groups.first().getByRole('button', { name: 'show', exact: true }).click();
	await expect(groups.first().locator('li')).toHaveCount(20);
	if (shots) await page.screenshot({ path: `${shots}/plan-b-warnings.png` });
	await groups.first().getByRole('button', { name: 'show all 71' }).click();
	await expect(groups.first().locator('li')).toHaveCount(71);
	await expect(groups.first().getByRole('button', { name: 'show all 71' })).toHaveCount(0);
	await groups.nth(1).getByRole('button', { name: 'show', exact: true }).click();
	await expect(groups.nth(1).locator('li')).toHaveCount(3);
	await expect(groups.nth(1).locator('li').first()).toContainText('requires source');
	expect((await calls(page)).filter((c) => c.section === 'warnings')).toEqual([
		{ section: 'warnings', offset: 0, limit: 20 },
		{ section: 'warnings', offset: 20, limit: 51 },
		{ section: 'warnings', offset: 71, limit: 3 }
	]);
});

test('an open warning group keeps its warnings while other sections load, and reads them once', async ({ page }) => {
	await openPlan(page);
	await select(page, 'warnings');
	const groups = pane(page).locator('.warning-group');
	await groups.first().getByRole('button', { name: 'show', exact: true }).click();
	await expect(groups.first().locator('li')).toHaveCount(20);
	// Another section's page joins the plan; the open group keeps what it read.
	await select(page, 'changed');
	await expect.poll(async () => (await calls(page)).some((c) => c.section !== 'warnings')).toBe(true);
	await select(page, 'warnings');
	await expect(groups.first().locator('li')).toHaveCount(20);
	// Shown, hidden and shown again before the first read returns: one read, each warning once.
	await groups.nth(1).getByRole('button', { name: 'show', exact: true }).evaluate((b: HTMLElement) => [b.click(), b.click(), b.click()]);
	await expect(groups.nth(1).locator('li')).toHaveCount(3);
	await page.waitForTimeout(300);
	await expect(groups.nth(1).locator('li')).toHaveCount(3);
	expect((await calls(page)).filter((c) => c.section === 'warnings')).toEqual([
		{ section: 'warnings', offset: 0, limit: 20 },
		{ section: 'warnings', offset: 71, limit: 3 }
	]);
	// Another plan's groups start closed and unread.
	await page.getByRole('button', { name: 'Plan again' }).click();
	await expect(groups.first().getByRole('button', { name: 'show', exact: true })).toBeVisible({ timeout: 30_000 });
	await expect(groups.locator('li')).toHaveCount(0);
});

test('the Checks tab under --changed leaves out the checks of nodes it skips', async ({ page }) => {
	const checksTab = async (cmd: string) => {
		await page.goto(`/plan?mock&cmd=${encodeURIComponent(cmd)}`);
		await expect(page.getByRole('button', { name: 'Plan again' })).toBeVisible({ timeout: 30_000 });
		const tab = page.getByRole('tablist', { name: 'View' }).getByRole('tab', { name: /Checks/ });
		await tab.click();
		return tab;
	};
	await expect(await checksTab('sqb build')).toHaveText(/Checks\s*13/);
	await expect(page.locator('.graph-note')).toHaveText('13 checks');
	// Seven of them check nodes --changed runs; the rest check nodes held back or unchanged.
	await expect(await checksTab('sqb build --changed')).toHaveText(/Checks\s*7/);
	await expect(page.locator('.graph-note')).toHaveText('7 checks');
});

test('seeds sit with the models of the same reason: first runs, changes and unchanged', async ({ page }) => {
	await openPlan(page, `/plan?mock&large&cmd=${encodeURIComponent('sqb build')}`);
	await select(page, 'first_run');
	await expect(pane(page).locator('.subtitle')).toHaveText('672 first run · 2 seeds');
	await select(page, 'changed');
	await expect(pane(page).locator('.subtitle')).toHaveText('87 query · 6 config · 1 function · 1 seed');
	await select(page, 'unchanged');
	await expect(pane(page).locator('.subtitle')).toHaveText('1,507 models · 12 functions · 7 seeds');
	// The header counts nodes as the index does, and the functions listed with them apart.
	await expect(pane(page).locator('h3')).toHaveText('Unchanged 1,514 nodes · 12 functions');
	await expect(page.locator('.index .entry[data-section="unchanged"] .count')).toHaveText('1,514');
	await expect(rows(page)).toHaveCount(50);
	if (shots) await page.screenshot({ path: `${shots}/plan-b-seeds.png` });
});

test('a small plan reads as a short index', async ({ page }) => {
	await openPlan(page, `/plan?mock&cmd=${encodeURIComponent('sqb build -s path:sql/models/marts')}`);
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
		await expect(pane(page).locator('.plan-row, [data-testid="plan-renames"] .row').first()).toBeVisible();
		await page.screenshot({ path: `${shots}/plan-b-${key}.png` });
	}
});

test('a failed read is not repeated on its own: it says so, and Retry reads again', async ({ page }) => {
	await page.addInitScript(() => ((window as unknown as { __sqbFailPlanRows?: boolean }).__sqbFailPlanRows = true));
	await page.goto(`${LARGE}&section=unchanged`);
	await expect(page.getByTestId('plan-rows-error')).toContainText('The plan has expired', { timeout: 30_000 });
	await page.waitForTimeout(1500);
	expect((await calls(page)).filter((c) => c.section === 'routine_models'), 'one read, not a retry loop').toHaveLength(1);
	await page.evaluate(() => ((window as unknown as { __sqbFailPlanRows?: boolean }).__sqbFailPlanRows = false));
	await page.getByTestId('plan-rows-error').getByRole('button', { name: 'Retry' }).click();
	await expect(rows(page)).toHaveCount(50);
	await expect(page.getByTestId('plan-rows-error')).toHaveCount(0);
});

test('a failed read blocks only its own group, and a re-plan reads afresh', async ({ page }) => {
	const fail = (on: boolean) => page.evaluate((v) => ((window as unknown as { __sqbFailPlanRows?: boolean }).__sqbFailPlanRows = v), on);
	await page.addInitScript(() => ((window as unknown as { __sqbFailPlanRows?: boolean }).__sqbFailPlanRows = true));
	await page.goto(`${LARGE}&section=unchanged`);
	await expect(page.getByTestId('plan-rows-error')).toBeVisible({ timeout: 30_000 });
	await fail(false);
	// Another group reads its rows, though Unchanged's read failed.
	await select(page, 'first_run');
	await expect(rows(page).first()).toBeVisible();
	await expect(page.getByTestId('plan-rows-error')).toHaveCount(0);
	// Back on Unchanged the failure stands until Retry, or until the plan is made again.
	await select(page, 'unchanged');
	await expect(page.getByTestId('plan-rows-error')).toBeVisible();
	await page.getByRole('button', { name: 'Plan again' }).click();
	await expect(rows(page)).toHaveCount(50, { timeout: 30_000 });
	await expect(page.getByTestId('plan-rows-error')).toHaveCount(0);
});
