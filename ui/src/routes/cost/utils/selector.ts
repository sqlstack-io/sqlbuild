// The Cost page's selection read as a place to stand: a free-form base (anything the tree can't
// break down), then a job, a folder and a model, each a comma part SQLBuild intersects. Drilling
// adds a part to every token; breadcrumbs take parts away. Pure.
import { parseSelection, type Part } from '$lib/selection/syntax';

/** A cost folder as `path:` selects it: SQL models under `sql/models/`, other roots (`python/…`, `sql/…`) as they are. */
export const pathOfFolder = (folder: string) => (/^(python|sql)\//.test(folder) ? folder : `sql/models/${folder}`);
/** A `path:` value as a cost folder: without `sql/models/` and a trailing slash. */
export const folderOfPath = (path: string) => path.replace(/^sql\/models(\/|$)/, '').replace(/\/$/, '');

export type Place = {
	/** The free-form tokens, each without the structured parts; empty when the tree covers it all. */
	base: string[];
	job: string | null;
	folder: string | null;
	model: string | null;
};

type Kind = 'job' | 'path' | 'name';
function kindOf(p: Part): Kind | null {
	if (p.up || p.down || p.between) return null;
	if (p.type === 'job') return 'job';
	if (p.type === 'path' || (p.type === null && p.value.includes('/'))) return 'path';
	if (p.type === null && !/[*?[]/.test(p.value)) return 'name';
	return null;
}

/** Splits a selection into its free-form base and the job, folder and model every token shares. */
export function placeOf(select: string, exclude = ''): Place {
	const tokens = parseSelection(select);
	const place: Place = { base: [], job: null, folder: null, model: null };
	if (!tokens.length) return place;
	if (exclude.trim()) return { ...place, base: tokens.map((t) => t.raw) };
	// The structured parts at the end of every token, the same in each.
	const tails = tokens.map((t) => {
		const parts = t.parts;
		let i = parts.length;
		while (i > 0 && kindOf(parts[i - 1])) i--;
		return { head: parts.slice(0, i), tail: parts.slice(i) };
	});
	const shared = tails[0].tail.filter((p) => tails.every((t) => t.tail.some((q) => q.raw === p.raw)));
	const kinds = shared.map((p) => kindOf(p)!);
	if (new Set(kinds).size !== kinds.length) return { ...place, base: tokens.map((t) => t.raw) };
	for (const p of shared) {
		const k = kindOf(p);
		if (k === 'job') place.job = p.value;
		else if (k === 'path') place.folder = folderOfPath(p.value);
		else place.model = p.value;
	}
	place.base = tails.map((t) => [...t.head, ...t.tail.filter((p) => !shared.some((s) => s.raw === p.raw))].map((p) => p.raw).join(',')).filter(Boolean);
	// A token that is nothing but the shared parts selects all of them, so nothing is shared after all.
	if (place.base.length !== tokens.length && (place.base.length || tokens.length > 1)) return { base: tokens.map((t) => t.raw), job: null, folder: null, model: null };
	return place;
}

/** The selection for a place: its parts joined to every base token, or on their own. */
export function selectorOf(place: Place): string {
	const parts = [place.job ? `job:${place.job}` : '', place.folder ? `path:${pathOfFolder(place.folder)}` : '', place.model ?? ''].filter(Boolean);
	if (!place.base.length) return parts.join(',');
	return place.base.map((b) => [b, ...parts].join(',')).join(' ');
}

/** `select` with another selection unioned in, as `sqb` reads space-separated tokens. */
export const union = (select: string, more: string) => [select.trim(), more].filter(Boolean).join(' ');

export type SelectorCrumb = { label: string; select: string; code?: boolean };

/** Breadcrumbs: All, the base, the job, each folder down, and the model, each the selection there. */
export function placeCrumbs(place: Place): SelectorCrumb[] {
	const out: SelectorCrumb[] = [{ label: 'All', select: '' }];
	let at: Place = { base: place.base, job: null, folder: null, model: null };
	if (place.base.length) out.push({ label: place.base.join(' '), select: selectorOf(at), code: true });
	if (place.job) out.push({ label: place.job, select: selectorOf((at = { ...at, job: place.job })) });
	if (place.folder) {
		const parts = place.folder.split('/');
		parts.forEach((p, i) => out.push({ label: p, select: selectorOf((at = { ...at, folder: parts.slice(0, i + 1).join('/') })) }));
	}
	if (place.model) out.push({ label: place.model, select: selectorOf({ ...at, model: place.model }) });
	return out;
}

