// Server selection evaluations and selector vocabularies, in the UI's shapes.

import type { components } from '$control/schema.d.ts';
import type { SelectionEvaluation, SelectionVocabulary, VocabularyEntry } from './types/operate';

type Wire = components['schemas'];

export function toSelectionEvaluation(wire: Wire['SelectionEvaluation']): SelectionEvaluation {
	const problem = wire.error ?? null;
	return {
		snapshotId: wire.snapshot_id ?? null,
		count: wire.count,
		keys: wire.keys,
		nextOffset: wire.next_offset ?? null,
		error: problem && {
			code: problem.code ?? null,
			message: problem.message,
			help: problem.help ?? null,
			location: problem.location ?? null,
			suggestions: problem.suggestions
		}
	};
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
