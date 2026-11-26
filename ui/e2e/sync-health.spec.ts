import { expect, test } from '@playwright/test';
import { FakeControl, SYNC_REASON } from './support/fake-control';

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
