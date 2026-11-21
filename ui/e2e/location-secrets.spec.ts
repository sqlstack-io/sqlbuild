import { expect, test } from '@playwright/test';

// A location's providers missing secrets (`?mock&secrets`: the shop has no orders API key; the dev target
// allows the export bucket's): a problem on the location and the Overview, a quiet note when allowed.
const SHOP = '0d9c6a52-3f1e-4b7a-9c43-5b8e2f61a7d4';
const DEV = 'c2f47d90-1b6e-4a85-93fd-6e0a5b28c171';

test('a location missing a provider secret says which, and the Overview lists it', async ({ page }) => {
	await page.goto(`/locations/${SHOP}?mock&secrets`);
	const problem = page.getByTestId('location-missing-secrets');
	await expect(problem).toContainText('Provider secrets missing · orders_api needs ORDERS_API_KEY', { timeout: 30_000 });
	await problem.locator('summary').click();
	await expect(problem).toContainText('ORDERS_API_KEY for api_key, line 18');
	await expect(problem.getByRole('link', { name: 'raw__orders' })).toBeVisible();

	await page.goto('/?mock&secrets');
	await expect(page.getByText("orders_api is missing ORDERS_API_KEY in the location's environment")).toBeVisible({ timeout: 30_000 });
});

test("a provider the target allows to miss its secrets is only a note", async ({ page }) => {
	await page.goto(`/locations/${DEV}?mock`);
	await expect(page.getByTestId('location-allowed-secrets')).toContainText('which this target allows (allow_missing_secrets)', { timeout: 30_000 });
	await expect(page.getByTestId('location-missing-secrets')).toHaveCount(0);
});
