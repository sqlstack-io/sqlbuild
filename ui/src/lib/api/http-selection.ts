// Server selection evaluations and selector vocabularies, in the UI's shapes.

import type { components } from '$control/schema.d.ts';
import type { SelectionEvaluation, SelectionProblem, SelectionVocabulary, VocabularyEntry } from './types/operate';

type Wire = components['schemas'];

export function toSelectionEvaluation(wire: Wire['SelectionEvaluation']): SelectionEvaluation {
	return {
		snapshotId: wire.snapshot_id ?? null,
		count: wire.count,
		keys: wire.keys,
		nextOffset: wire.next_offset ?? null,
		error: toSelectionProblem(wire.error)
	};
}

/** Why the server refuses a selection, in the UI's shape; null when it doesn't. */
export function toSelectionProblem(problem: Wire['SelectionProblem'] | null | undefined): SelectionProblem | null {
	return problem
		? {
				code: problem.code ?? null,
				message: problem.message,
				help: problem.help ?? null,
				location: problem.location ?? null,
				suggestions: problem.suggestions,
				refused: problem.refused ? toRefusal(problem.refused) : null
			}
		: null;
}

/** The server's refused part in the UI's shape: its index as `part`, and a bare name or glob as type `null`. */
function toRefusal(refused: Wire['SelectionRefusal']): NonNullable<SelectionEvaluation['error']>['refused'] {
	const bare = refused.type === 'name' || refused.type === 'glob';
	return { part: refused.index, type: bare ? null : refused.type, value: refused.value };
}

export function toSelectionVocabulary(wire: Wire['SelectionVocabulary']): SelectionVocabulary {
	const entries = (list: Wire['VocabularyEntry'][]): VocabularyEntry[] => list.map(({ value, count }) => ({ value, count }));
	return {
		snapshotId: wire.snapshot_id ?? null,
		tags: entries(wire.tags),
		paths: entries(wire.paths),
		jobs: entries(wire.jobs),
		sources: entries(wire.sources),
		seeds: entries(wire.seeds),
		kinds: entries(wire.kinds)
	};
}
