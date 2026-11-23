// Rerun choices with their disabled reasons, for the header's split button.

import type { Capability, RunDetail } from '$lib/api';
import { isActiveRun } from '$lib/status';
import type { RerunChoice } from '../types';
import { plural } from '$lib/format/plural';


export function rerunChoices(detail: RunDetail, selected: readonly string[], can: (c: Capability) => boolean): RerunChoice[] {
	const { run } = detail;
	const needs: Capability[] = run.rewriteHistory ? ['run.rerun', 'run.rewrite_history'] : ['run.rerun'];
	const missing = needs.filter((c) => !can(c));
	const common = isActiveRun(run.status)
		? 'Wait for the run to finish, or cancel it first'
		: missing.length
			? `You don't have permission to rerun this (${missing.join(', ')})`
			: null;
	const failures = detail.rerunFromFailureNodes.length;
	const choices: RerunChoice[] = [];
	if (selected.length) {
		choices.push({
			mode: 'selected',
			label: `Rerun ${plural(selected.length, 'selected node')}`,
			help: `Runs only ${selected.slice(0, 3).join(', ')}${selected.length > 3 ? '…' : ''}`,
			disabledReason: common
		});
	}
	choices.push({
		mode: 'from_failure',
		label: 'Rerun from failure',
		help: failures ? `${plural(failures, 'unfinished node')}` : 'Every node finished',
		disabledReason: common ?? (failures ? null : 'Nothing failed, was cancelled or was skipped in this run')
	});
	choices.push({
		mode: 'all',
		label: 'Rerun all',
		help: `Whole ${run.kind === 'group' ? 'group' : run.kind === 'adhoc' ? 'selection' : 'job'}`,
		disabledReason: common
	});
	return choices;
}
