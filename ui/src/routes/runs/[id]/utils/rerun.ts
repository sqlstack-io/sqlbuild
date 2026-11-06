// Rerun choices with their disabled reasons, for the header's split button.

import type { Capability, RunDetail } from '$lib/api';
import { isActiveRun } from '$lib/status';
import type { RerunChoice } from '../types';

const plural = (n: number, noun: string) => `${n} ${noun}${n === 1 ? '' : 's'}`;

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
		help: failures ? `Runs the ${plural(failures, 'node')} that failed or were skipped because of a failure` : 'Nothing failed',
		disabledReason: common ?? (failures ? null : 'Nothing failed or was skipped in this run')
	});
	choices.push({
		mode: 'all',
		label: 'Rerun all',
		help: `Runs the whole ${run.kind === 'group' ? 'group' : run.kind === 'adhoc' ? 'selection' : 'job'} again`,
		disabledReason: common
	});
	return choices;
}

/** The split button's main action: from failure after a failure, otherwise all. */
export function primaryChoice(detail: RunDetail, choices: RerunChoice[]): RerunChoice {
	const failedLike = detail.run.status === 'failed' || detail.run.status === 'partial' || detail.run.status === 'presumed_failed';
	const wanted = failedLike && detail.rerunFromFailureNodes.length ? 'from_failure' : 'all';
	return choices.find((c) => c.mode === wanted)!;
}
