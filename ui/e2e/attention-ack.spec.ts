import { expect, test } from '@playwright/test';

// The Overview's attention list: every item can be acknowledged. A run is hidden for good; an ongoing
// condition until it changes. Both move to "Acknowledged (N)", where anyone can undo them.
test.use({ viewport: { width: 1600, height: 1000 } });

test('acknowledging a condition moves it under Acknowledged until it changes, and Undo brings it back', async ({ page }) => {
	await page.goto('/?mock');
	const issues = page.locator('section.health .issues');
	const row = issues.locator('.issue', { hasText: 'past the freshness threshold' });
	await expect(row).toBeVisible({ timeout: 20_000 });

	const ack = row.getByRole('button', { name: 'Acknowledge' });
	await expect(ack).toHaveText('Ack');
	await expect(ack).toHaveAttribute('title', /until it changes/);
	await ack.click();
	await expect(row).toHaveCount(0);

	const handled = page.locator('section.health .handled');
	await expect(handled.getByRole('button', { name: /^Acknowledged \(1\)$/ })).toBeVisible();
	await handled.getByRole('button', { name: /^Acknowledged/ }).click();
	const acknowledged = handled.locator('.issue', { hasText: 'past the freshness threshold' });
	await expect(acknowledged).toContainText('Acknowledged by you');
	await expect(acknowledged).toContainText('hidden until it changes');

	await acknowledged.getByRole('button', { name: 'Undo' }).click();
	await expect(issues.locator('.issue', { hasText: 'past the freshness threshold' })).toBeVisible();
	await expect(handled).toHaveCount(0);
});

test('a run is acknowledged for good', async ({ page }) => {
	await page.goto('/?mock');
	const row = page.locator('section.health .issues .issue', { hasText: 'core_build partial' });
	await expect(row).toBeVisible({ timeout: 20_000 });
	await expect(row.getByRole('button', { name: 'Acknowledge' })).toHaveAttribute('title', /^Hide this from the attention list for everyone viewing this target\./);
	await row.getByRole('button', { name: 'Acknowledge' }).click();
	const handled = page.locator('section.health .handled');
	await handled.getByRole('button', { name: /^Acknowledged/ }).click();
	await expect(handled.locator('.issue', { hasText: 'core_build partial' })).toContainText('for good');
});
