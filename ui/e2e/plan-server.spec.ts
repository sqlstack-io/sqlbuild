import { expect, test } from '@playwright/test';
import { FakeControl } from './support/fake-control';
import { FakePlans, renamesPlan } from './support/fake-plans';

// The plan page against a faked control server, through the real server client: a field the client
// drops between the server's summary and the page shows here, where the mock client would hide it.
// Covers the summary's migration_steps and the rows' origin_model, old_name and is_rename.
test.use({ viewport: { width: 1600, height: 1000 } });

async function open(page: import('@playwright/test').Page, plans: FakePlans) {
	await new FakeControl().install(page);
	await plans.install(page);
	await page.goto(`/plan?cmd=${encodeURIComponent('sqb build')}&section=renames`);
	await expect(page.getByTestId('plan-renames')).toBeVisible({ timeout: 30_000 });
}

test('the Renames and migrations section reads the server’s step count and rename fields', async ({ page }) => {
	const plans = new FakePlans(renamesPlan());
	await open(page, plans);
	const renames = page.getByTestId('plan-renames');
	// migration_steps from the summary: the forced move and the column rename, not the two renames' own migrations.
	await expect(renames.locator('h3')).toHaveText('Renames and migrations 2 nodes · 2 migration steps');
	await expect(page.getByTestId('index-will-run')).toHaveText('Will run3');
	await expect(page.locator('.index .entry[data-section="renames"] .count')).toHaveText('2');
	// origin_model and old_name from the renamed rows.
	const renamed = renames.locator('[data-part="renamed"] .row');
	await expect(renamed).toHaveCount(2);
	await expect(renamed.first().locator('.from')).toHaveText('customers');
	await expect(renamed.first().locator('.detail')).toHaveText('old table archived; a view at the old name until 2026-12-18');
	await expect(renamed.nth(1).locator('.from')).toHaveText('payments');
	// is_rename leaves the renames' own migrations out of the steps.
	const moved = renames.locator('[data-kind="migrations"]');
	await expect(moved.locator('h5')).toHaveText('Data moved between models 1');
	await expect(moved.locator('.row')).toHaveCount(1);
	await expect(moved.locator('.row')).toContainText('replaced by the data of orders_legacy (forced)');
	await expect(renames.locator('[data-kind="column_migrations"] .row')).toContainText('amt → amount');
});

test('the filter asks the server, which finds a renamed model by its old name', async ({ page }) => {
	const plans = new FakePlans(renamesPlan());
	await open(page, plans);
	const renames = page.getByTestId('plan-renames');
	await renames.getByRole('textbox', { name: 'Filter renames and migrations' }).fill('payments');
	await expect(renames.locator('[data-part="renamed"] .row')).toHaveCount(1);
	await expect(renames.locator('[data-part="renamed"] .row .from')).toHaveText('payments');
	expect(plans.reads.some((r) => r.section === 'renamed' && r.name === 'payments')).toBe(true);
});
