// SQLBuild-aware syntax highlighting.
//
// SQLBuild SQL is a superset of SQL: a MODEL(...) config header (materialization,
// tags, description, columns + audits DSL) followed by a SELECT body, with
// __ref()/__source() reference functions. No stock grammar knows the header, so
// we layer a small SQLBuild grammar on top of Prism's base SQL grammar.
//
// Output is class-based (.token.*), themed via CSS variables in layout.css so it
// adapts to light/dark automatically.

import Prism from 'prismjs';
import 'prismjs/components/prism-sql';

let registered = false;

export function ensureSqlbuildGrammar() {
	if (registered) return;
	registered = true;

	// clone Prism's base SQL grammar as the body language
	const base = Prism.languages.sql;

	const sqlbuild = Prism.languages.extend('sql', {});
	// must exist before insertBefore('sqlbuild', ...) can extend it
	Prism.languages.sqlbuild = sqlbuild;

	// SQLBuild reference functions: __ref('x'), __source('a','b') -> function color
	Prism.languages.insertBefore('sqlbuild', 'function', {
		'sb-ref': {
			pattern: /\b__(?:ref|source|var|env)\b(?=\s*\()/,
			alias: 'function'
		}
	});

	// MODEL config-header keywords (the DSL before SELECT)
	Prism.languages.insertBefore('sqlbuild', 'keyword', {
		'sb-config': {
			// MODEL / materialization kinds / config props / audit names
			pattern:
				/\b(?:MODEL|materialized|view|table|incremental|ephemeral|seed|source|loader|tags|description|columns|audits|owner|tags|cursor|unique_key|grain|cluster_by|partition_by|strategy|merge|delete_insert|append|microbatch|not_null|unique|accepted_values|relationships|expression_is_true|freshness)\b/i,
			alias: 'keyword'
		}
	});

	// register under both names so `language-sqlbuild` and `language-sql` both work
	void base;
}

export function highlightSqlbuild(code: string): string {
	ensureSqlbuildGrammar();
	return Prism.highlight(code, Prism.languages.sqlbuild, 'sqlbuild');
}
