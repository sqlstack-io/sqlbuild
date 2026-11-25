import { describe, expect, it } from 'vitest';
import type { SelectionProblem, SelectionVocabulary } from '$lib/api';
import { backspace, choose, comma, commit, EMPTY, fieldOf, fieldText, finished, replacePart, replaceToken, toggleExpansion, type FieldState } from './field';
import { readingText } from './reading';
import { parseSelection } from './syntax';
import { pickedValues, setValuesIn, toggleValueIn } from './picker';
import { buildHref, copyCommand, runsNamed, problemParts, problemText, problemToken, readSelectionParams, writeSelectionParams } from './command';
import { suggest } from './suggest';

const typing = (s: FieldState, text: string) => ({ ...s, text });

describe('selection syntax', () => {
	it('splits tokens on spaces and parts on commas, with + on its own part', () => {
		const [t] = parseSelection('+tag:finance,path:models/marts');
		expect(t.parts.map((p) => [p.type, p.value, p.up, p.down])).toEqual([
			['tag', 'finance', true, false],
			['path', 'models/marts', false, false]
		]);
		expect(parseSelection('a  b+').map((t) => [t.raw, t.start])).toEqual([['a', 0], ['b+', 3]]);
		expect(parseSelection('+a~b+')[0].parts[0]).toMatchObject({ between: ['a', 'b'], up: true, down: true });
	});
});

// Each row is how `sqb` resolves the selection: spaces union, commas intersect, `+` widens the part it
// is written on before the intersection, `a~b` is a path, a bare name with `/` is a folder, and the
// except row is taken from the whole.
const READINGS: [string, string, string][] = [
	['', '', 'every node'],
	['tag:finance', '', 'nodes tagged finance'],
	['+tag:finance', '', 'nodes tagged finance and everything upstream of them'],
	['orders+', '', 'orders and everything downstream of it'],
	['+orders+', '', 'orders and everything upstream and downstream of it'],
	['+tag:finance,path:models/marts', '', 'nodes tagged finance and everything upstream of them, that are also in models/marts'],
	['tag:finance,+path:models/marts', '', 'nodes tagged finance, that are also among nodes in models/marts and everything upstream of them'],
	['tag:a,tag:b,kind:model', '', 'nodes tagged a, that are also tagged b and models'],
	['tag:a tag:b', '', 'nodes tagged a, plus nodes tagged b'],
	['models/staging', '', 'nodes in models/staging'],
	['stg_*', '', 'nodes named like stg_*'],
	['orders~revenue', '', 'nodes on the paths from orders to revenue'],
	['job:nightly', 'tag:deprecated', 'the nodes of job nightly, minus nodes tagged deprecated'],
	['', 'kind:seed source:raw', 'every node, minus seeds or source raw']
];

describe('reads as', () => {
	it.each(READINGS)('%s --exclude %s', (select, exclude, reading) => {
		expect(readingText(select, exclude)).toBe(reading);
	});
});

describe('selection field', () => {
	it('turns a finished token into a chip and Backspace turns it back into its exact text', () => {
		let s = commit(typing(EMPTY, '+tag:finance'));
		expect(s).toEqual({ tokens: ['+tag:finance'], parts: [], text: '' });
		s = backspace(s);
		expect(s).toEqual({ tokens: [], parts: [], text: '+tag:finance' });
		expect(fieldText(s)).toBe('+tag:finance');
		expect(finished(s)).toBe('');
	});

	it('joins comma parts into one token, and a comma after a chip reopens it', () => {
		let s = comma(typing(EMPTY, 'tag:a'));
		s = commit(typing(s, 'path:b'));
		expect(s.tokens).toEqual(['tag:a,path:b']);
		s = comma(s);
		expect(s).toEqual({ tokens: [], parts: ['tag:a', 'path:b'], text: '' });
		s = commit(typing(s, 'kind:model'));
		expect(finished(s)).toBe('tag:a,path:b,kind:model');
	});

	it('peels an and-group one part at a time on Backspace', () => {
		let s = backspace(fieldOf('x tag:a,path:b'));
		expect(s).toEqual({ tokens: ['x'], parts: ['tag:a'], text: 'path:b' });
		s = backspace({ ...s, text: '' });
		expect(s).toEqual({ tokens: ['x'], parts: [], text: 'tag:a' });
	});

	it('keeps text and chips in sync through edits', () => {
		let s = fieldOf('tag:a,path:b c');
		s = toggleExpansion(s, 0, 0, 'up');
		expect(finished(s)).toBe('+tag:a,path:b c');
		s = replacePart(s, 0, 1, '');
		expect(finished(s)).toBe('+tag:a c');
		s = replaceToken(s, 1, 'd e');
		expect(finished(s)).toBe('+tag:a d e');
		s = choose(s, 'tag:', false);
		expect(s.text).toBe('tag:');
		expect(finished(choose(s, 'tag:z', true))).toBe('+tag:a d e tag:z');
		expect(finished(commit(typing(EMPTY, 'a b')))).toBe('a b');
	});
});

describe('chip value picker', () => {
	it('ticks more values as copies of the token, which sqb unions', () => {
		let tokens = ['+tag:a,path:x', 'other'];
		tokens = toggleValueIn(tokens, 0, 0, 'b');
		expect(tokens).toEqual(['+tag:a,path:x', '+tag:b,path:x', 'other']);
		expect(pickedValues(tokens, 0, 0)).toEqual(['a', 'b']);
		expect(pickedValues(tokens, 1, 0)).toEqual(['a', 'b']);
		expect(toggleValueIn(tokens, 1, 0, 'a')).toEqual(['+tag:b,path:x', 'other']);
	});

	it('sets the picked values at once, as Select all and Clear do', () => {
		const tokens = ['tag:a', 'x'];
		expect(setValuesIn(tokens, 0, 0, ['a', 'b', 'c'])).toEqual(['tag:a', 'tag:b', 'tag:c', 'x']);
		expect(setValuesIn(['tag:a', 'tag:b', 'tag:c', 'x'], 1, 0, ['c'])).toEqual(['tag:c', 'x']);
		expect(setValuesIn(['tag:a', 'x'], 0, 0, [])).toEqual(['x']);
	});
});

describe('selection outside the bar', () => {
	const problem: SelectionProblem = { code: 'S008', message: "no nodes found with tag 'finanse'", help: null, location: { field: 'select', token: '+tag:finanse,path:x', start: 2, end: 21 }, suggestions: ['+tag:finance,path:x'], refused: null };

	it('round-trips through the URL, and reads an old single-node link', () => {
		const params = writeSelectionParams(new URLSearchParams('fields=a'), { select: '+tag:a,path:b c', exclude: 'tag:x' });
		expect(readSelectionParams(new URLSearchParams(params.toString()))).toEqual({ select: '+tag:a,path:b c', exclude: 'tag:x' });
		expect(writeSelectionParams(params, { select: '', exclude: '' }).toString()).toBe('fields=a');
		expect(readSelectionParams(new URLSearchParams('select=orders'))).toEqual({ select: 'orders', exclude: '' });
	});

	it('copies a shell command and opens the plan page with the same selection', () => {
		expect(copyCommand({ select: '+tag:a,path:b stg_*', exclude: 'tag:x' })).toBe("sqb build -s +tag:a,path:b 'stg_*' --exclude tag:x");
		expect(decodeURIComponent(buildHref({ select: '+tag:a stg_*', exclude: '' }))).toBe('/plan?cmd=sqb build -s +tag:a stg_*');
		expect(decodeURIComponent(buildHref({ select: 'a', exclude: 'b c' }))).toBe('/plan?cmd=sqb build -s a --exclude b c');
	});

	it('finds the token and part a refusal points at', () => {
		expect(problemToken(problem, 'select', 'a +tag:finanse,path:x')).toBe(1);
		expect(problemToken(problem, 'exclude', 'a +tag:finanse,path:x')).toBeNull();
		expect(problemParts(problem, '+tag:finanse,path:x')).toEqual([0]);
		expect(problemParts({ ...problem, message: 'refused' }, '+tag:finanse,path:x')).toEqual([0, 1]);
		expect(problemParts({ ...problem, message: 'refused', refused: { part: 1, type: 'path', value: 'x' } }, '+tag:finanse,path:x')).toEqual([1]);
	});

	it.each([
		[{ part: 0, type: 'tag', value: 'finanse' }, 'No tag called finanse.'],
		[{ part: 0, type: null, value: 'x' }, 'No node called x.'],
		[{ part: 0, type: 'path', value: 'models/x' }, 'No folder models/x.'],
		[{ part: 0, type: null, value: 'models/x' }, 'No folder models/x.'],
		[{ part: 0, type: 'job', value: 'nightly' }, 'No job called nightly.'],
		[{ part: 0, type: 'kind', value: 'modle' }, 'No node kind called modle.'],
		[{ part: 0, type: null, value: 'stg_*' }, 'Nothing matches stg_*.']
	])('words a refused %o as the design does', (refused, text) => {
		expect(problemText({ ...problem, refused })).toBe(text);
	});

	it("falls back to sqb's message when the refused part is unknown", () => {
		expect(problemText(problem)).toBe("no nodes found with tag 'finanse'");
	});
});

const VOCAB: SelectionVocabulary = {
	snapshotId: 's',
	tags: [
		{ value: 'finance', count: 5 },
		{ value: 'staging', count: 9 },
		{ value: 'daily', count: 2 }
	],
	paths: [{ value: 'models/staging', count: 9 }],
	jobs: [],
	sources: [],
	seeds: [],
	kinds: [{ value: 'model', count: 20 }],
	audits: [{ value: 'revenue_reconciles', count: 1 }]
};

describe('suggestions', () => {
	it('offers recent selections, the selector types and popular tags for an empty field', () => {
		const groups = suggest('', VOCAB, [], [{ select: 'tag:a', count: 3 }]);
		expect(groups.map((g) => g.name)).toEqual(['Recent', 'Selectors', 'Popular tags']);
		expect(groups[2].items.map((i) => i.label)).toEqual(['staging', 'finance', 'daily']);
	});

	it('offers a type, its values and node names for a fragment, keeping a leading +', () => {
		const groups = suggest('+sta', VOCAB, [{ value: 'stg_stage', hint: 'models' }]);
		expect(groups.map((g) => g.name)).toEqual(['Tags', 'Folders', 'Nodes']);
		expect(groups[0].items[0]).toMatchObject({ insert: '+tag:staging', count: 9, complete: true });
		const ta = suggest('ta', VOCAB, []);
		expect(ta[0].items[0]).toMatchObject({ insert: 'tag:', complete: false });
		expect(ta[1]).toMatchObject({ name: 'Tags' });
		expect(ta[1].items.map((i) => i.insert)).toEqual(['tag:finance', 'tag:staging', 'tag:daily']);
	});

	it('offers only that type’s values after a colon', () => {
		expect(suggest('tag:fin', VOCAB, []).flatMap((g) => g.items.map((i) => i.insert))).toEqual(['tag:finance']);
	});

	it('offers audit names after audit:, as an audit selector takes them', () => {
		expect(suggest('audit:rev', VOCAB, []).flatMap((g) => g.items.map((i) => i.insert))).toEqual(['audit:revenue_reconciles']);
	});
});

describe('runs named', () => {
	it.each([
		{ description: 'run selectors in any position, with + and comma parts, once each', text: 'build -s failed:abc12345+ tag:x,run:abc12345 --exclude skipped:def67890', expected: ['abc12345', 'def67890'] },
		{ description: 'no run selector names no run', text: 'build -s audit:a_matches_c model:b', expected: [] }
	])('$description', ({ text, expected }) => {
		expect(runsNamed(text)).toEqual(expected);
	});
});
