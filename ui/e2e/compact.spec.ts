import { expect, test } from '@playwright/test';
import { zoomTo } from './zoom';

// Zoomed out, cards take their compact form: a one-line pill, a ring in the colour of the card's state
// holding its kind's icon, then the part of its name that tells it apart from its siblings. The name
// never wraps, shows at 12 px or more on screen, shrinks to fit before it is cut, and is cut only at its
// end. Cards are drawn in full down to 0.82.
// With SHOTS set, each graph is also photographed at each zoom: SHOTS=/some/dir npx playwright test e2e/compact.spec.ts
const shots = process.env.SHOTS;

const GRAPHS: { name: string; path: string; click?: string }[] = [
	{ name: 'group', path: '/jobs/warehouse_nightly?mock&awkward' },
	{ name: 'group-members', path: '/jobs/warehouse_nightly?mock&awkward', click: 'Folders' },
	{ name: 'lineage-overview', path: '/lineage?mock&large' },
	// Opened, its models' names share endings across layers and folders.
	{ name: 'lineage-overview-open', path: '/lineage?mock&large', click: 'Expand all' },
	{ name: 'lineage', path: '/lineage?mock' }
];

/** Each compact pill wholly in view that breaks a rule, as problems. */
function pillProblems(): string[] {
	const problems: string[] = [];
	const flow = document.querySelector('.svelte-flow')!.getBoundingClientRect();
	const zoom = Number(/scale\(([\d.]+)\)/.exec(document.querySelector<HTMLElement>('.svelte-flow__viewport')!.style.transform)?.[1] ?? 1);
	const root = getComputedStyle(document.documentElement);
	// Colours compared as the browser resolves them.
	const probe = document.body.appendChild(document.createElement('i'));
	const colour = (value: string) => ((probe.style.color = ''), (probe.style.color = value.trim()), getComputedStyle(probe).color);
	const token = (name: string) => colour(root.getPropertyValue(name));
	const tones = Object.fromEntries(['--sb-success', '--sb-error', '--sb-running', '--sb-warning', '--sb-stale', '--sb-behind', '--sb-text-faint'].map((t) => [t, token(t)]));
	/** The card's state colour, as its full card shows it: its last outcome, unless it needs building. */
	function expected(card: HTMLElement): string | null {
		const status = card.querySelector<HTMLElement>(':scope > .status');
		if (card.classList.contains('group')) {
			const dots = [...card.querySelectorAll<HTMLElement>('.status i')].map((i) => i.style.background);
			const has = (t: string) => dots.some((d) => d.includes(t));
			return has('--sb-error') ? tones['--sb-error'] : has('--sb-stale') ? tones['--sb-stale'] : has('--sb-success') ? tones['--sb-success'] : tones['--sb-text-faint'];
		}
		if (!status) return null;
		const tone = colour(getComputedStyle(status).getPropertyValue('--tone'));
		const outcomeFirst = tone === tones['--sb-error'] || tone === tones['--sb-running'] || (card.querySelector('.needs') && tone !== tones['--sb-success'] && tone !== tones['--sb-warning']);
		if (outcomeFirst) return tone;
		if (card.querySelector('.drift.behind, .needs .behind:only-child')) return tones['--sb-behind'];
		if (card.querySelector('.drift, .needs .changed')) return tones['--sb-stale'];
		return tone;
	}
	const visible: string[] = [];
	for (const node of document.querySelectorAll<HTMLElement>('.svelte-flow__node')) {
		const card = node.firstElementChild as HTMLElement;
		const pill = card?.querySelector<HTMLElement>(':scope > .cface .pill');
		if (!pill) continue;
		const box = pill.getBoundingClientRect();
		if (box.left < flow.left || box.right > flow.right || box.top < flow.top || box.bottom > flow.bottom) continue;
		const full = pill.title;
		const name = [...pill.querySelectorAll<HTMLElement>('.nm')].find((n) => getComputedStyle(n).display !== 'none');
		if (!name) {
			problems.push(`${full}: no name shown`);
			continue;
		}
		visible.push(`${pill.dataset.parent ?? ''} / ${name.textContent ?? ''}`);
		const px = parseFloat(getComputedStyle(name).fontSize) * zoom;
		if (px < 11.95) problems.push(`${full}: its name shows at ${px.toFixed(2)} px`);
		if (Math.abs(box.height - 30) > 1) problems.push(`${full}: its pill is ${box.height.toFixed(1)} px tall`);
		// One line: never wrapped, one line box.
		if (getComputedStyle(name).whiteSpace !== 'nowrap' || name.getClientRects().length !== 1 || name.getBoundingClientRect().height > px * 1.6) problems.push(`${full}: its name wraps`);
		// Cut only once it can shrink no further, and then at its end.
		if (name.scrollWidth > name.clientWidth + 0.5) {
			if (px > 12.05) problems.push(`${full}: cut at ${px.toFixed(2)} px, before shrinking to 12`);
			if (getComputedStyle(name).textOverflow !== 'ellipsis') problems.push(`${full}: cut without an ellipsis at its end`);
		}
		if (!full.endsWith(name.textContent ?? '\0')) problems.push(`${full}: shows "${name.textContent}", not an ending of its name`);
		const ring = pill.querySelector<HTMLElement>('.ring')!;
		const want = expected(card);
		if (want && getComputedStyle(ring).borderTopColor !== want) problems.push(`${full}: its ring is ${getComputedStyle(ring).borderTopColor}, its state ${want}`);
	}
	probe.remove();
	const repeated = visible.filter((t, i) => visible.indexOf(t) !== i);
	if (repeated.length) problems.push(`names shown more than once among siblings: ${[...new Set(repeated)].join(', ')}`);
	if (!visible.length) problems.push('no compact card in view');
	return problems;
}

test.use({ viewport: { width: 1600, height: 900 } });

for (const { name, path, click } of GRAPHS) {
	test(`${name}: zoomed out, each card is a one-line pill with its state's ring`, async ({ page }) => {
		test.setTimeout(90_000);
		await page.goto(path);
		await page.locator('.graph-canvas:not(.unfitted) .svelte-flow__node').first().waitFor({ timeout: 20_000 });
		if (click) await page.getByRole('button', { name: click }).click();
		await page.waitForTimeout(800);
		const canvas = page.locator('.graph-canvas').first();

		// Just above the compact zoom, cards are still drawn in full.
		await zoomTo(page, 0.85);
		await expect(canvas).not.toHaveClass(/\bcompact\b/);
		if (shots) await page.screenshot({ path: `${shots}/compact-${name}-z085-full.png` });

		for (const zoom of [0.7, 0.5, 0.4]) {
			await zoomTo(page, zoom);
			await expect(canvas).toHaveClass(/\bcompact\b/);
			if (shots) await page.screenshot({ path: `${shots}/compact-${name}-z0${Math.round(zoom * 100)}.png` });
			expect(await page.evaluate(pillProblems), `${name} at ${zoom}`).toEqual([]);
		}
	});
}

test('a job pill’s play triangle sits in the middle of its ring', async ({ page }) => {
	await page.goto('/jobs/warehouse_nightly?mock');
	await page.locator('.graph-canvas:not(.unfitted) .svelte-flow__node').first().waitFor({ timeout: 20_000 });
	await page.waitForTimeout(800);
	for (const zoom of [0.7, 0.5]) {
		await zoomTo(page, zoom);
		const offsets = await page.evaluate(() =>
			[...document.querySelectorAll<HTMLElement>('.pill[data-kind="job"] .ring')].map((ring) => {
				// The triangle's corners: two on its left edge, one at its right tip; its centroid a third of the way across.
				const path = ring.querySelector<SVGGraphicsElement>('svg path')!;
				const b = path.getBBox();
				const m = path.getScreenCTM()!;
				const centroid = new DOMPoint(b.x + b.width / 3, b.y + b.height / 2).matrixTransform(m);
				const r = ring.getBoundingClientRect();
				return { dx: centroid.x - (r.left + r.width / 2), dy: centroid.y - (r.top + r.height / 2), size: r.width };
			})
		);
		expect(offsets.length, 'job pills in view').toBeGreaterThan(0);
		for (const o of offsets) {
			expect(Math.abs(o.dx), `horizontally, in a ${o.size.toFixed(1)} px ring at ${zoom}`).toBeLessThanOrEqual(0.5);
			expect(Math.abs(o.dy), `vertically, at ${zoom}`).toBeLessThanOrEqual(0.5);
		}
	}
});
