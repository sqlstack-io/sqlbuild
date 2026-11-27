import { expect, test } from '@playwright/test';
import { FakeControl, LOCATIONS, SYNC_REASON } from './support/fake-control';

// A failing run-history sync is named in the top bar and on the attention list, and leaves both once it
// works again; a sync failed reads stopped can be resumed from the list.
test.use({ viewport: { width: 1920, height: 1000 } });

test('a sync that can’t reach the warehouse is named in the top bar and needs attention until it recovers', async ({ page }) => {
	const control = new FakeControl({ shop: { failingSync: { unreachable: true, halted: false } } });
	await control.install(page);
	await page.goto('/');
	const status = page.getByTestId('sync-status');
	await expect(status).toHaveText(new RegExp(`^Run history can't reach the warehouse since (\\d{4}-\\d{2}-\\d{2} )?\\d{2}:\\d{2} · retrying every 60 s · ${SYNC_REASON.replace(/[()]/g, '\\$&')}$`), { timeout: 30_000 });
	await expect(status).toHaveAttribute('href', /\/locations\//);
	const issues = page.locator('.issues');
	await expect(issues).toContainText("run history can't reach the warehouse since", { timeout: 30_000 });
	await expect(page.getByTestId('attention-quiet')).toHaveCount(0);
	control.targets.shop.failingSync = null;
	control.notify('all');
	await expect(issues).toHaveCount(0, { timeout: 30_000 });
	await expect(status).not.toContainText('Run history');
});

test('a sync that failed reads stopped can be resumed from the attention list', async ({ page }) => {
	const control = new FakeControl({ shop: { failingSync: { unreachable: false, halted: true } } });
	await control.install(page);
	await page.goto('/');
	await expect(page.getByTestId('sync-status')).toContainText('Run history sync stopped since', { timeout: 30_000 });
	const resume = page.getByTestId('attention-resume');
	await expect(resume).toBeVisible({ timeout: 30_000 });
	await resume.click();
	await expect.poll(() => control.writes.filter((w) => w.path === '/warehouse-syncs').length).toBe(1);
});

test('a sync report that arrives after the location page loaded updates it without a reload', async ({ page }) => {
	const control = new FakeControl({ shop: { sync: 'failed' } });
	await control.install(page);
	await page.goto(`/locations/${LOCATIONS.shop}:dev`);
	const line = page.locator('.sync-now');
	await expect(line).toContainText('failing', { timeout: 30_000 });
	await page.evaluate(() => Object.assign(window, { stillLoaded: true }));
	const syncNow = line.getByRole('button', { name: 'Sync now' });
	await syncNow.click();
	await expect.poll(() => control.writes.filter((w) => w.path === '/warehouse-syncs').length).toBe(1);
	await expect(line).toContainText('syncing…');
	await expect(syncNow).toBeDisabled();
	// The location's successful report reaches the page as the server's change feed announces it.
	control.targets.shop.sync = 'settled';
	control.notify(`warehouse_sync:${LOCATIONS.shop}`);
	await expect(line).toContainText(/synced (just now|\d+s ago)/, { timeout: 30_000 });
	await expect(syncNow).toBeEnabled();
	expect(await page.evaluate(() => (window as unknown as { stillLoaded?: boolean }).stillLoaded)).toBe(true);
});
