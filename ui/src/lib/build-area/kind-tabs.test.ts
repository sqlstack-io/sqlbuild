import { describe, expect, it } from 'vitest';
import { NODE_KINDS, kindTabOf, nodeIcon, nodeIconKey, parseKindTab } from './labels';

describe('catalog kind tabs', () => {
	it('puts Python nodes and functions in their own tabs', () => {
		expect(['asset', 'loader'].map((k) => kindTabOf(k as never))).toEqual(['python', 'python']);
		expect(['udf', 'table_fn'].map((k) => kindTabOf(k as never))).toEqual(['functions', 'functions']);
		expect(kindTabOf('view')).toBe('view');
	});

	it('reads a tab or a kind from a link, and nothing else', () => {
		expect(parseKindTab('python')).toBe('python');
		expect(parseKindTab('loader')).toBe('python');
		expect(parseKindTab('table_fn')).toBe('functions');
		expect(parseKindTab('audit')).toBeNull();
		expect(parseKindTab(null)).toBeNull();
	});

	it('has an icon for every node kind', () => {
		for (const kind of NODE_KINDS) expect(nodeIcon[nodeIconKey({ kind, materialization: null })], kind).toBeDefined();
	});
});
