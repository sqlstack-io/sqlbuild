import { expect, test, type Page } from '@playwright/test';

// Timing guards. They run after the layout suite, alone (see playwright.config.ts), so other tests
// competing for the machine don't blur the timings.

async function settle(page: Page) {
	await page.waitForLoadState('networkidle');
	await page.waitForTimeout(500);
}

test.use({ viewport: { width: 1440, height: 1000 } });

test('Checks opens again quickly from the sidebar with thousands of checks', async ({ page }) => {
	const nav = (name: string) => page.locator('nav a', { hasText: name }).first();
	const openChecks = async () => {
		const started = Date.now();
		await nav('Checks').click();
		await page.locator('table.sb-table tbody tr:not(.spacer)').first().waitFor();
		await page.waitForLoadState('networkidle');
		return Date.now() - started;
	};
	await page.goto('/?mock&awkward');
	await settle(page);
	await openChecks();
	await nav('Overview').click();
	await settle(page);
	// The first visit loads the list; the second took 12–19 s on a large project when filter options
	// were counted by filtering the whole list once per option.
	expect(await openChecks(), 'ms for the second visit to Checks to settle').toBeLessThan(2000);
});
