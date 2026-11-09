import { describe, expect, it } from 'vitest';
import type { SnapshotNode } from '$lib/api';
import { commandTokens, suggest, withVerb } from './command';

const node = (key: string, kind: SnapshotNode['kind'], path: string, tags: string[] = []): SnapshotNode => ({ key, kind, group: '', tags, path, cursor: null, upstream: [] });

const NODES: SnapshotNode[] = [
	node('model:customers', 'table', 'models/marts/customers.sql', ['finance']),
	node('seed:countries', 'seed', 'seeds/countries.csv'),
	node('source:shop.orders', 'source', 'sources.yml')
];

describe('the plan page command line', () => {
	it('sends the tokens after sqb exactly as typed, for the server to resolve', () => {
		expect(commandTokens('  sqb build customers+ --exclude tag:finance   --warehouse WH_S ')).toEqual(['build', 'customers+', '--exclude', 'tag:finance', '--warehouse', 'WH_S']);
	});

	it('suggests selectors in the CLI grammar: bare model names, seed: and source: keys, tags and folders', () => {
		const values = (typed: string) => suggest(typed, typed.length, NODES).map((s) => s.value);
		expect(values('sqb build --select +cust')).toEqual(['+customers']);
		expect(values('sqb build --select countr')).toEqual(['seed:countries']);
		expect(values('sqb build --select shop.ord')).toEqual(['source:shop.orders']);
		expect(values('sqb build --select fin')).toEqual(['tag:finance']);
		expect(values('sqb build --select marts')).toEqual(['path:models/marts']);
	});

	it('keeps the selection and only the flags the new verb accepts when switching verbs', () => {
		expect(withVerb('sqb build --select customers --full-refresh --warehouse WH_S', 'test')).toBe('sqb test --select customers --warehouse WH_S ');
	});
});
