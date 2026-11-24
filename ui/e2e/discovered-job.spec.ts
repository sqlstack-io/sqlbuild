import { expect, test } from '@playwright/test';

// A job whose nodes come from a discovered set: the graph frames the set's steps under a header that
// collapses it, cards read by step, and the side panel shows the set, or a step's item and the set.
for (const width of [2000, 1280]) {
	test(`at ${width} px a discovered set is framed, named by its steps, and nothing overflows`, async ({ page }) => {
		await page.setViewportSize({ width, height: 1000 });
		await page.goto('/jobs/forecast_v2_eu_job?mock');
		const header = page.getByRole('button', { name: 'Collapse discovered set forecast_eu_plan' });
		await expect(header).toBeVisible({ timeout: 20_000 });
		await expect(header).toContainText('discovered set · 36 steps');
		await expect(page.locator('.svelte-flow__node[data-id="asset:forecast_eu[gbm_chain_d1]"]')).toContainText('gbm_chain_d1');
		const panel = page.locator('.insp-sec', { hasText: 'Discovered set' }).first();
		await expect(panel).toContainText('forecast_eu_plan');
		await expect(panel).toContainText('Steps');
		const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
		expect(overflow).toBeLessThanOrEqual(0);
	});
}

test('the set header collapses the set to one node and opens it again', async ({ page }) => {
	await page.goto('/jobs/forecast_v2_eu_job?mock');
	const collapse = page.getByRole('button', { name: 'Collapse discovered set forecast_eu_plan' });
	await collapse.click({ timeout: 20_000 });
	const expand = page.getByRole('button', { name: 'Expand discovered set forecast_eu_plan' });
	await expect(expand).toHaveAttribute('aria-expanded', 'false');
	await expect(page.locator('.svelte-flow__node[data-id="asset:forecast_eu[gbm_chain_d1]"]')).toHaveCount(0);
	await expand.focus();
	await page.keyboard.press('Enter');
	await expect(page.getByRole('button', { name: 'Collapse discovered set forecast_eu_plan' })).toHaveAttribute('aria-expanded', 'true');
});

test('a selected step shows its item and its set, and the failed set its error', async ({ page }) => {
	await page.goto('/jobs/forecast_v2_us_job?mock');
	await page.locator('.svelte-flow__node[data-id="asset:forecast_us[gbm_chain_d1]"]').click({ timeout: 20_000 });
	const rail = page.locator('.rail');
	await expect(rail.getByText('Item', { exact: true })).toBeVisible();
	await expect(rail).toContainText('depth');
	await expect(rail).toContainText('Fetch failed');
	await expect(rail).toContainText('503 Service Unavailable');
});
