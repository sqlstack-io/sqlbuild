import { expect, test } from '@playwright/test';
import { zoomTo } from './zoom';

// Zoomed out, cards are drawn in stages: in full down to 0.8; then node cards show their name alone,
// enlarged to about 10 px on screen, while other cards show their pill; then every card is a one-line
// pill, a dot in the colour of the card's state and the part of its name that tells it apart from its
// siblings; then the dot alone. A name never wraps, shrinks to fit before it is cut, and is cut at its
// end, except a discovered step's, which first drops the prefix it shares with its siblings.
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
	const stage = document.querySelector<HTMLElement>('.graph-canvas')!.dataset.stage;
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
	const inView = (box: DOMRect) => box.left >= flow.left && box.right <= flow.right && box.top >= flow.top && box.bottom <= flow.bottom;
	/** Whether `text` is `whole` or `whole` cut as the stages cut it: its end, or a shared prefix and then the rest's end. */
	const cutRight = (text: string, whole: string) => {
		if (text === whole) return true;
		const kept = text.replace(/…$/, '');
		const joint = (at: number) => at > 0 && /[_.[/:-]/.test(whole[at - 1]);
		if (!text.startsWith('…') && !text.endsWith('…')) {
			// A shared prefix cut short, then the rest that tells it apart whole: `hier…supplier`.
			const [head, rest] = text.split('…');
			return whole.startsWith(head) && whole.endsWith(rest) && joint(whole.length - rest.length);
		}
		if (!text.startsWith('…')) return whole.startsWith(kept);
		const rest = kept.slice(1);
		return joint(whole.indexOf(rest));
	};
	const oneLine = (el: HTMLElement, px: number) => getComputedStyle(el).whiteSpace === 'nowrap' && el.getClientRects().length === 1 && el.getBoundingClientRect().height <= px * 1.8;
	for (const node of document.querySelectorAll<HTMLElement>('.svelte-flow__node')) {
		const card = node.firstElementChild as HTMLElement;
		const pill = card?.querySelector<HTMLElement>(':scope > .cface .pill');
		// A name-only card: its own frame, its name alone at about 10 px on screen, on one line, fitted.
		const nameOnly = node.classList.contains('svelte-flow__node-asset') ? card?.querySelector<HTMLElement>(':scope > .head .name') : null;
		if (nameOnly && (!pill || !pill.getClientRects().length)) {
			if (!inView(card.getBoundingClientRect()) || stage !== 'name') continue;
			const text = nameOnly.textContent ?? '';
			const px = parseFloat(getComputedStyle(nameOnly).fontSize) * zoom;
			visible.push(`${node.parentElement?.dataset.id ?? ''} / ${text}`);
			if (px < 9.5) problems.push(`${nameOnly.title}: its name shows at ${px.toFixed(2)} px`);
			if (!oneLine(nameOnly, px)) problems.push(`${nameOnly.title}: its name wraps`);
			if (nameOnly.scrollWidth > nameOnly.clientWidth + 0.5) problems.push(`${nameOnly.title}: "${text}" overflows its card`);
			if (!cutRight(text, nameOnly.dataset.label ?? '')) problems.push(`${nameOnly.title}: shows "${text}", not its name cut at the end`);
			continue;
		}
		if (!pill || !pill.getClientRects().length) continue;
		const box = pill.getBoundingClientRect();
		if (!inView(box)) continue;
		const full = pill.title;
		const dot = pill.querySelector<HTMLElement>('.dot')!;
		const want = expected(card);
		if (want && getComputedStyle(dot).backgroundColor !== want) problems.push(`${full}: its dot is ${getComputedStyle(dot).backgroundColor}, its state ${want}`);
		const name = pill.querySelector<HTMLElement>('.nm')!;
		if (stage === 'dot') {
			if (getComputedStyle(name).display !== 'none') problems.push(`${full}: a dot shows its name`);
			visible.push(`dot ${visible.length}`);
			continue;
		}
		const text = name.textContent ?? '';
		const tail = name.dataset.tail ?? '';
		visible.push(`${pill.dataset.parent ?? ''} / ${text}`);
		const px = parseFloat(getComputedStyle(name).fontSize) * zoom;
		if (px < 9) problems.push(`${full}: its name shows at ${px.toFixed(2)} px`);
		if (!oneLine(name, px)) problems.push(`${full}: its name wraps`);
		if (name.scrollWidth > name.clientWidth + 0.5) problems.push(`${full}: "${text}" overflows its pill`);
		if (!full.endsWith(tail) || !cutRight(text, tail)) problems.push(`${full}: shows "${text}", not its tail "${tail}" cut at the end`);
	}
	probe.remove();
	if (!visible.length) problems.push('no compact card in view');
	return problems;
}

test.use({ viewport: { width: 1600, height: 900 } });

for (const { name, path, click } of GRAPHS) {
	test(`${name}: zoomed out, cards show their name alone, then a one-line pill with its state's dot, then the dot`, async ({ page }) => {
		test.setTimeout(90_000);
		await page.goto(path);
		await page.locator('.graph-canvas:not(.unfitted) .svelte-flow__node').first().waitFor({ timeout: 20_000 });
		if (click) await page.getByRole('button', { name: click }).click();
		await page.waitForTimeout(800);
		const canvas = page.locator('.graph-canvas').first();

		// Just above the name-only stage, cards are still drawn in full.
		await zoomTo(page, 0.82);
		await expect(canvas).not.toHaveClass(/\bcompact\b/);
		if (shots) await page.screenshot({ path: `${shots}/compact-${name}-z085-full.png` });

		for (const [zoom, stage] of [[0.7, 'name'], [0.45, 'name'], [0.35, 'pill'], [0.32, 'pill'], [0.25, 'dot']] as const) {
			await zoomTo(page, zoom);
			await expect(canvas).toHaveAttribute('data-stage', stage);
			if (shots) await page.screenshot({ path: `${shots}/compact-${name}-z0${Math.round(zoom * 100)}.png` });
			expect(await page.evaluate(pillProblems), `${name} at ${zoom}`).toEqual([]);
		}
	});
}

test('a selected pill stands out from the rest', async ({ page }) => {
	await page.goto('/lineage?mock');
	await page.locator('.graph-canvas:not(.unfitted) .svelte-flow__node').first().waitFor({ timeout: 20_000 });
	await page.waitForTimeout(800);
	await zoomTo(page, 0.34);
	const pills = page.locator('.cface .pill');
	// Two pills wholly in view of the graph, so neither sits under the sidebar.
	const [one, two] = await pills.evaluateAll((all) => {
		const flow = document.querySelector('.graph-canvas')!.getBoundingClientRect();
		return all.flatMap((p, i) => {
			const b = p.getBoundingClientRect();
			return b.left >= flow.left && b.right <= flow.right && b.top >= flow.top && b.bottom <= flow.bottom ? [i] : [];
		});
	});
	const looks = (i: number) => pills.nth(i).evaluate((p) => ({ border: getComputedStyle(p).borderTopColor, ring: getComputedStyle(p).boxShadow, fill: getComputedStyle(p).backgroundColor }));
	const before = await looks(one);
	await pills.nth(one).click();
	const [selected, other] = [await looks(one), await looks(two)];
	const primary = await page.evaluate(() => { const i = document.body.appendChild(document.createElement('i')); i.style.color = getComputedStyle(document.documentElement).getPropertyValue('--primary'); const c = getComputedStyle(i).color; i.remove(); return c; });
	expect(selected.border).toBe(primary);
	expect(selected.ring).not.toBe('none');
	expect(selected.fill).not.toBe(before.fill);
	expect(other.border).not.toBe(primary);
});
