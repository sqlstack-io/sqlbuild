import { expect, test } from '@playwright/test';

// Audit failures a build lets through (`ignore_audit_failures`): a success with a warning, never a
// failure. In the mock, core_build ignores audit failures and its run run-2b70 let two through.
// With SHOTS set, each surface is photographed: SHOTS=/some/dir npx playwright test e2e/ignored-audits.spec.ts
const shots = process.env.SHOTS;
test.use({ viewport: { width: 1600, height: 1000 } });

test('the run list reads a run that let audits fail as a success with a warning', async ({ page }) => {
	await page.goto('/runs?mock&q=2b70');
	const row = page.locator('tr', { has: page.locator('a', { hasText: '2b70' }) });
	const status = row.getByTestId('run-status');
	await expect(status).toHaveText('Succeeded · 2 audit failures ignored', { timeout: 20_000 });
	// Warning-toned, not red, and unlike a plain success.
	const colours = (e: Element) => ({ tone: getComputedStyle(e).getPropertyValue('--tone').trim(), warning: getComputedStyle(document.documentElement).getPropertyValue('--sb-warning').trim(), error: getComputedStyle(document.documentElement).getPropertyValue('--sb-error').trim() });
	const ignored = await status.evaluate(colours);
	expect(ignored.tone).toBe(ignored.warning);
	expect(ignored.tone).not.toBe(ignored.error);
	if (shots) await page.screenshot({ path: `${shots}/ignored-audits-runs.png` });
	await page.goto('/runs?mock&q=91cd');
	const plain = page.locator('tr', { has: page.locator('a', { hasText: '91cd' }) }).getByTestId('run-status');
	await expect(plain).toHaveText('Succeeded', { timeout: 20_000 });
	const success = await plain.evaluate(colours);
	expect(success.tone).not.toBe(success.warning);
});

test('the run page lists each ignored failure by model and audit', async ({ page }) => {
	await page.goto('/runs/run-2b70?mock');
	await expect(page.getByTestId('run-status')).toHaveText('Succeeded · 2 audit failures ignored', { timeout: 20_000 });
	const panel = page.getByTestId('ignored-audits');
	await expect(panel.locator('.head')).toContainText('2 audit failures ignored');
	await expect(panel.locator('li')).toHaveCount(2);
	await expect(panel.locator('li', { hasText: 'fact_orders' })).toContainText('unique(order_id)');
	await expect(panel.locator('li', { hasText: 'dim_customers' })).toContainText('unique(customer_id)');
	await expect(panel.locator('li .tag')).toHaveText(['ignored', 'ignored']);
	if (shots) await page.screenshot({ path: `${shots}/ignored-audits-run.png` });
	// A run that let nothing through has no such list.
	await page.goto('/runs/run-91cd?mock');
	await expect(page.getByTestId('run-status')).toHaveText('Succeeded', { timeout: 20_000 });
	await expect(page.getByTestId('ignored-audits')).toHaveCount(0);
});

test('the Checks page filters to the failures builds let through', async ({ page }) => {
	await page.goto('/checks?mock');
	await page.getByRole('tab', { name: /^Ignored failures/ }).click({ timeout: 20_000 });
	const rows = page.locator('table.checks tbody tr, .checks tbody tr');
	await expect(rows).toHaveCount(2);
	await expect(rows.locator('.ignored')).toHaveText(['ignored', 'ignored']);
	if (shots) await page.screenshot({ path: `${shots}/ignored-audits-checks.png` });
});

test('a job that ignores audit failures says so in its settings', async ({ page }) => {
	await page.goto('/jobs/core_build?mock');
	await expect(page.getByTestId('job-ignores-audit-failures')).toHaveText("Audit failures don't stop runs", { timeout: 20_000 });
	await page.goto('/jobs/marts_only?mock');
	await expect(page.locator('.concurrency')).toBeVisible({ timeout: 20_000 });
	await expect(page.getByTestId('job-ignores-audit-failures')).toHaveCount(0);
});

test('the plan says when audit failures won’t stop the build', async ({ page }) => {
	await page.goto(`/plan?mock&cmd=${encodeURIComponent('sqb build --ignore-audit-failures')}`);
	await expect(page.getByTestId('plan-ignores-audit-failures')).toHaveText('Audit failures won’t stop this build'.replace('’', "'"), { timeout: 30_000 });
	if (shots) await page.screenshot({ path: `${shots}/ignored-audits-plan.png` });
	await page.goto(`/plan?mock&cmd=${encodeURIComponent('sqb build')}`);
	await expect(page.getByRole('button', { name: 'Plan again' })).toBeVisible({ timeout: 30_000 });
	await expect(page.getByTestId('plan-ignores-audit-failures')).toHaveCount(0);
});

test('the run list shows the whole label, unclipped', async ({ page }) => {
	await page.goto('/runs?mock&q=2b70');
	const status = page.locator('tr', { has: page.locator('a', { hasText: '2b70' }) }).getByTestId('run-status');
	await expect(status).toBeVisible({ timeout: 20_000 });
	const cell = status.locator('xpath=ancestor::td[1]');
	const [s, c] = [await status.boundingBox(), await cell.boundingBox()];
	expect(s!.x + s!.width).toBeLessThanOrEqual(c!.x + c!.width);
	expect(await status.evaluate((e) => e.scrollWidth <= e.clientWidth)).toBe(true);
	expect(await cell.evaluate((e) => e.scrollWidth <= e.clientWidth), 'the cell is not cut off').toBe(true);
});
