// Server cost reads of a selection, in the UI's shapes.

import type { components } from '$control/schema.d.ts';
import { toSelectionProblem } from './http-selection';
import { priceSource } from './http-runs';
import type { CostSelection, CostSeriesGroup } from './types/operate';

type Wire = components['schemas'];

export function toCostSelection(wire: Wire['CostSelectionRecord']): CostSelection {
	const series = (list: Wire['CostSeriesRecord'][]) => list.map((s) => ({ key: s.key ?? null, credits: s.credits }));
	return {
		snapshotId: wire.snapshot_id ?? null,
		since: wire.since,
		days: wire.days,
		dates: wire.dates,
		error: toSelectionProblem(wire.error),
		price: wire.price ? { usdPerCredit: wire.price.usd_per_credit, source: priceSource(wire.price.source, true) ?? 'config' } : null,
		credits: wire.credits,
		previousCredits: wire.previous_credits,
		usd: wire.usd ?? null,
		previousUsd: wire.previous_usd ?? null,
		runs: wire.runs,
		incompleteRuns: wire.incomplete_runs,
		queries: wire.queries,
		seconds: wire.seconds,
		bytesScanned: wire.bytes_scanned,
		facts: wire.facts.map((f) => ({
			job: f.job ?? null,
			nodeKey: f.node_key,
			folder: f.folder,
			credits: f.credits,
			previousCredits: f.previous_credits,
			usd: f.usd ?? null,
			previousUsd: f.previous_usd ?? null,
			seconds: f.seconds,
			runs: f.runs,
			queries: f.queries,
			bytesScanned: f.bytes_scanned
		})),
		jobDaily: series(wire.job_daily),
		folderDaily: series(wire.folder_daily)
	};
}

/** Groups as the series read takes them, in its JSON. */
export const costSeriesGroups = (groups: CostSeriesGroup[]) =>
	JSON.stringify(groups.map((g) => ({ job: g.job ?? null, manual: g.manual ?? false, folder: g.folder ?? null, node_key: g.nodeKey ?? null })));
