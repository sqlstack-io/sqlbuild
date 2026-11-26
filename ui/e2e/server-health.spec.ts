import { expect, test } from '@playwright/test';
import { FakeControl, LOCATIONS } from './support/fake-control';

// The shell against a faked server whose control database fails: health answers, every other read is a 503
// naming the cause. The page keeps its URL, says why, and loads by itself once the database is back.
test.use({ viewport: { width: 1440, height: 900 } });

const DNS = "Can't reach the control database: control-db.internal doesn't resolve (DNS)";
const since = () => new Date(Date.now() - 12 * 60_000).toISOString();

test('a control database outage keeps the page, says why and loads it by itself on recovery', async ({ page }) => {
	const control = new FakeControl();
	control.outage = { case: 'name_lookup', message: DNS, since: since() };
	await control.install(page);
	await page.goto('/runs');
	const outage = page.getByTestId('outage');
	await expect(outage.locator('h1')).toHaveText("Runs isn't available", { timeout: 30_000 });
	await expect(page.getByTestId('outage-reason')).toHaveText(DNS);
	await expect(outage).toContainText('Checking again every 1 s');
	await expect(outage.locator('tr[data-component="control_database"]')).toContainText('Unavailable');
	await expect(page.getByTestId('server-indicator')).toContainText('Unavailable');
	await expect(page.locator('aside a', { hasText: 'Runs' })).toHaveAttribute('aria-disabled', 'true');
	await expect(page.getByTestId('nav-status')).not.toHaveAttribute('aria-disabled', 'true');
	await page.evaluate(() => Object.assign(window, { stayedLoaded: true }));
	control.outage = null;
	await expect(outage).toHaveCount(0, { timeout: 15_000 });
	await expect(page).toHaveURL(/\/runs$/);
	await expect(page.locator('.app-topbar h1')).toHaveText('Runs');
	await expect(page.getByTestId('server-indicator')).not.toContainText('Unavailable');
	expect(await page.evaluate(() => (window as unknown as { stayedLoaded?: boolean }).stayedLoaded)).toBe(true);
});

test('each control database failure reaches the page by its own message', async ({ page }) => {
	const cases = [
		['name_lookup', DNS],
		['unreachable', "Can't reach the control database at control-db.internal:5432"],
		['login_rejected', 'The control database rejected the login for sqb'],
		['no_free_connections', 'The control database has no free connections'],
		['restarting', 'The control database is restarting'],
		['pool_exhausted', 'All 10 control database connections were busy for 30 s'],
		['locked', 'The control database is locked by another process'],
		['other', 'Control database error: could not read block 7 in file "base/16384/2619"']
	];
	const control = new FakeControl();
	control.outage = { case: cases[0][0], message: cases[0][1], since: since() };
	await control.install(page);
	await page.goto('/');
	const shown: string[] = [];
	for (const [kind, message] of cases) {
		control.outage = { case: kind, message, since: since() };
		await expect(page.getByTestId('outage-reason')).toHaveText(message, { timeout: 15_000 });
		shown.push((await page.getByTestId('outage-reason').textContent()) ?? '');
	}
	expect(shown).toEqual(cases.map(([, message]) => message));
});

test('a degraded part is named in a banner that links to Status, and pages still work', async ({ page }) => {
	const control = new FakeControl();
	const silent = "Hasn't reported since 2026-11-25 17:40 UTC";
	control.degraded = [{ component: 'location', name: 'returns', status: 'degraded', reason: silent, location_id: LOCATIONS.returns }];
	await control.install(page);
	await page.goto('/runs');
	const banner = page.getByTestId('health-banner');
	await expect(banner).toContainText(`returns: ${silent}`, { timeout: 30_000 });
	await expect(page.getByTestId('server-indicator')).toContainText('Degraded');
	await expect(page.getByTestId('outage')).toHaveCount(0);
	await banner.getByRole('link', { name: 'View status →' }).click();
	await expect(page).toHaveURL(/\/status$/);
	await expect(page.getByTestId('health-table').locator('tr', { hasText: 'returns' })).toContainText(silent);
});

test('Status shows every part without a session', async ({ page }) => {
	const control = new FakeControl();
	control.signedOut = true;
	await control.install(page);
	await page.goto('/status');
	const table = page.getByTestId('health-table');
	await expect(table.locator('tr[data-component="control_database"]')).toContainText('Healthy', { timeout: 30_000 });
	await expect(table.locator('tr[data-component="location"]')).toHaveCount(2);
	await expect(page).toHaveURL(/\/status$/);
});
