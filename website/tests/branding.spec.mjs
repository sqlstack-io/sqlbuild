import { test, expect } from '@playwright/test';

for (const path of ['/', '/docs/quickstart/']) {
	for (const width of [320, 390, 800, 1440]) {
		for (const theme of ['light', 'dark']) {
			test(`${path} ${width}px ${theme}: branding and header offsets`, async ({ page }) => {
				await page.setViewportSize({ width, height: 900 });
				await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
				await page.goto(path);
				await page.evaluate(() => document.fonts.ready);
				await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
				await expect(page.getByRole('link', { name: 'by SQLStack', exact: true })).toHaveAttribute('href', 'https://sqlstack.io');
				await expect(page.getByRole('link', { name: 'Part of SQLStack', exact: true })).toHaveAttribute('href', 'https://sqlstack.io');
				const products = page.getByRole('navigation', { name: 'SQLStack products' });
				await expect(products.getByRole('link')).toHaveText(['SQLBuild', 'SQLClean', 'SQLTide', 'Fensu']);
				for (const [name, href] of Object.entries({ SQLBuild: '/', SQLClean: 'https://sqlclean.io', SQLTide: 'https://docs.streambuild.dev', Fensu: 'https://docs.fensu.dev' })) {
					await expect(products.getByRole('link', { name, exact: true })).toHaveAttribute('href', href);
				}
				const metrics = await page.evaluate(() => {
					const box = (selector) => document.querySelector(selector).getBoundingClientRect();
					const header = box('.site-header');
					const brand = box('.brand-group');
					const byline = box('.brand-byline');
					const controls = box('.nav, .search');
					return {
						top: brand.top - header.top,
						bottom: header.bottom - brand.bottom,
						centerDifference: Math.abs((brand.top + brand.bottom - controls.top - controls.bottom) / 2),
						textOffset: byline.left - brand.left,
						overflow: document.documentElement.scrollWidth - innerWidth,
						visibleSymbols: [...document.querySelectorAll('.sqlstack-parent img')].filter((img) => getComputedStyle(img).display !== 'none').map((img) => new URL(img.src).pathname),
					};
				});
				// The entire 54px group needs breathing room, not just the logo.
				expect(metrics.top).toBeGreaterThanOrEqual(12);
				expect(metrics.bottom).toBeGreaterThanOrEqual(12);
				expect(metrics.centerDifference).toBeLessThanOrEqual(1);
				expect(metrics.textOffset).toBeCloseTo(31.85, 0);
				expect(metrics.overflow).toBeLessThanOrEqual(0);
				expect(metrics.visibleSymbols).toEqual([`/assets/sqlstack${theme === 'dark' ? '-dark' : ''}.png`]);
				for (const image of await page.locator('.brand-group img, .sqlstack-parent img').all()) {
					expect(await image.evaluate((img) => img.complete && img.naturalWidth > 0)).toBe(true);
				}
				if (path === '/') {
					const howLink = page.getByRole('link', { name: 'How it works', exact: true });
					if (await howLink.isVisible()) {
						// Keep anchor geometry deterministic without sampling smooth-scroll frames.
						await page.evaluate(() => { document.documentElement.style.scrollBehavior = 'auto'; });
						await howLink.click();
					} else {
						await page.goto('/#how');
					}
					await expect(page.locator('#how h2')).toBeInViewport();
					const headerBottom = await page.locator('.site-header').evaluate((el) => el.getBoundingClientRect().bottom);
					expect((await page.locator('#how h2').boundingBox()).y).toBeGreaterThanOrEqual(headerBottom);
				} else {
					await expect(page.getByRole('navigation', { name: 'Pagination' })).toBeVisible();
					const anchor = page.locator('.sl-markdown-content h2[id]').first();
					const id = await anchor.getAttribute('id');
					await page.goto(`${path}#${id}`);
					await expect(anchor).toBeInViewport();
					const headerBottom = await page.locator('.site-header').evaluate((el) => el.getBoundingClientRect().bottom);
					expect((await anchor.boundingBox()).y).toBeGreaterThanOrEqual(headerBottom);
					if (width < 800) {
						await page.getByRole('button', { name: 'Menu', exact: true }).click();
						const sidebar = page.locator('#starlight__sidebar');
						await expect(sidebar).toBeVisible();
						expect((await sidebar.boundingBox()).y).toBeGreaterThanOrEqual(headerBottom - 1);
						await page.keyboard.press('Escape');
						await expect(sidebar).not.toBeVisible();
					}
				}
			});
		}
	}
}
