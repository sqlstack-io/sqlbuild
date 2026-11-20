import { expect, test, type Page } from '@playwright/test';
import { FakeControl } from './support/fake-control';

// The Concurrency panel on job and group pages: plain facts about what runs alongside what. With
// SHOTS set, each page is photographed: SHOTS=/some/dir npx playwright test e2e/concurrency.spec.ts
const shots = process.env.SHOTS;
test.use({ viewport: { width: 1600, height: 1000 } });

/** The panel's lines, as text, once the page has loaded; a group page opens its details first. */
async function panel(page: Page, job: string, group = false): Promise<string[]> {
	await page.goto(`/jobs/${job}?mock`);
	if (group) await page.getByRole('button', { name: 'Toggle details' }).click({ timeout: 20_000 });
	const lines = page.locator('ul.concurrency > li');
	await expect(lines.first()).toBeVisible({ timeout: 20_000 });
	await lines.last().scrollIntoViewIfNeeded();
	if (shots) await page.screenshot({ path: `${shots}/concurrency-panel-${job}.png` });
	// Each fact, without the runs holding a pool now, which change as the mock's runs do.
	return (await lines.allInnerTexts()).map((t) => t.replace(/\s+/g, ' ').replace(/ Running now: .*$/, '').trim());
}

test('a job states its run concurrency limit, overlap, and its pool', async ({ page }) => {
	const lines = await panel(page, 'core_build');
	expect(lines).toEqual([
		"Inside a run: at most 8 of this job's nodes at once",
		'Never alongside runs writing the same tables (automatic)',
		'If triggered while running: is skipped',
		'warehouse_heavy: takes 1 of 1 (capped on this server; the project allows 3), shared with reporting_exports, close_of_business'
	]);
	await expect(page.getByRole('link', { name: 'How concurrency works' })).toHaveAttribute('href', 'https://sqlbuild.com/docs/concepts/concurrency/');
	await expect(page.locator('ul.concurrency a', { hasText: 'reporting_exports' })).toHaveAttribute('href', /\/jobs\/reporting_exports/);
});

test('a job without its own setting states the limit its runs get, without naming where it is set', async ({ page }) => {
	for (const [job, line] of [
		['marts_only', "Inside a run: at most 6 of this job's nodes at once"],
		['activity_microbatch', "Inside a run: at most 2 of this job's nodes at once"],
		['revenue_refresh', "Inside a run: at most 4 of this job's nodes at once"],
		['load_sources', "Inside a run: at most 4 of this job's nodes at once"]
	] as const) {
		expect((await panel(page, job))[0], job).toBe(line);
	}
});

test('a job in two pools lists each, and a job that waits says so', async ({ page }) => {
	const lines = await panel(page, 'reporting_exports');
	expect(lines).toContain('If triggered while running: waits');
	expect(lines).toContain('supplier_api: takes 1 of 2, shared with load_sources');
	expect(lines.filter((l) => l.includes(': takes '))).toHaveLength(2);
});

test('a group capped below what it asks takes the whole pool, shown as a warning', async ({ page }) => {
	const lines = await panel(page, 'close_of_business', true);
	expect(lines).toContain('Takes the whole warehouse_heavy pool on this server (capped at 1; the project asks for 3), shared with core_build, reporting_exports');
	await expect(page.locator('ul.concurrency li.pool.clamped')).toHaveCount(1);
});

test('a group in no pool says so', async ({ page }) => {
	const lines = await panel(page, 'nightly_all', true);
	expect(lines).toContain('Pools: none');
	expect(lines[0]).toBe('Members without their own limit: at most 4 nodes at once');
});

test("the location shows the server's pool-cap warning", async ({ page }) => {
	await page.goto('/locations/0d9c6a52-3f1e-4b7a-9c43-5b8e2f61a7d4?mock');
	const caps = page.locator('.pool-caps');
	await expect(caps).toContainText('Pool caps on this server', { timeout: 20_000 });
	await expect(caps).toContainText('group close_of_business takes 3 of pool warehouse_heavy; capped at 1, so it runs alone in that pool');
	await expect(caps.getByRole('link', { name: 'close_of_business' })).toHaveAttribute('href', /\/jobs\/close_of_business/);
	if (shots) await page.screenshot({ path: `${shots}/concurrency-panel-location.png` });
});

test("a published job's limit comes through the server's job read as a plain limit", async ({ page }) => {
	const server = new FakeControl({
		shop: {
			jobs: [
				{ key: 'orders_job', kind: 'job', concurrency: 2, limit: 2 },
				{ key: 'refunds_job', kind: 'job', concurrency: null, limit: 16 },
				{ key: 'nightly', kind: 'group', concurrency: null, limit: 16 }
			]
		}
	});
	await server.install(page);
	for (const [job, line, group] of [
		['orders_job', "Inside a run: at most 2 of this job's nodes at once", false],
		['refunds_job', "Inside a run: at most 16 of this job's nodes at once", false],
		['nightly', 'Members without their own limit: at most 16 nodes at once', true]
	] as const) {
		await page.goto(`/jobs/${job}`);
		if (group) await page.getByRole('button', { name: 'Toggle details' }).click({ timeout: 20_000 });
		const first = page.locator('ul.concurrency > li').first();
		await expect(first, job).toHaveText(line, { timeout: 20_000 });
	}
});
