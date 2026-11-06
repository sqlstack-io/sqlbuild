// Pure helpers for the SQL viewer: structural outline, compiled rendering and
// line-level highlighting with clickable __ref() links.

import { highlightSqlbuild } from './highlight';

export type OutlineItem = { label: string; line: number };
export type SqlOutline = {
	config: { key: string; value: string }[];
	refs: OutlineItem[];
	steps: OutlineItem[];
	finalSelect?: number;
	columns: string[];
};

const REF_RE = /__ref\(\s*"(\w+)"\s*\)/g;

export function compileRefs(raw: string, database = 'waffle_shop', schema = 'prod'): string {
	const withoutHeader = raw.replace(/^MODEL\s*\([\s\S]*?\n\);\s*\n+/, '');
	return withoutHeader.replace(REF_RE, (_m, name: string) => `"${database}"."${schema}"."${name}"`);
}

export function outlineSql(raw: string): SqlOutline {
	const lines = raw.split('\n');
	const config: { key: string; value: string }[] = [];
	const header = raw.match(/^MODEL\s*\(([\s\S]*?)\n\);/);
	if (header) {
		for (const key of ['materialized', 'strategy', 'unique_key', 'tags', 'owner']) {
			const m = header[1].match(new RegExp(`\\b${key}\\s+(\\[[^\\]]*\\]|[^,\\n]+)`));
			if (m) config.push({ key, value: m[1].replace(/[[\]"]/g, '').replace(/,\s*/g, ', ').trim() });
		}
	}

	const refs: OutlineItem[] = [];
	const seen = new Set<string>();
	const steps: OutlineItem[] = [];
	let finalSelect: number | undefined;
	lines.forEach((line, i) => {
		for (const m of line.matchAll(REF_RE)) {
			if (!seen.has(m[1])) {
				seen.add(m[1]);
				refs.push({ label: m[1], line: i + 1 });
			}
		}
		const cte = line.match(/^\s*(?:WITH\s+|,\s*)?(\w+)\s+AS\s*\($/i);
		if (cte) steps.push({ label: cte[1], line: i + 1 });
		if (/^SELECT\b/i.test(line)) finalSelect = i + 1;
	});

	const columns: string[] = [];
	if (finalSelect) {
		for (const line of lines.slice(finalSelect)) {
			if (/^\s*FROM\b/i.test(line)) break;
			const col = line.trim().replace(/,$/, '');
			if (!col) continue;
			const alias = col.match(/\bAS\s+(\w+)$/i)?.[1] ?? col.split('.').pop()?.split(/\s+/).pop();
			if (alias && /^\w+$/.test(alias)) columns.push(alias);
		}
	}
	return { config, refs, steps, finalSelect, columns };
}

/** Highlight SQL and split into per-line HTML; raw refs become clickable links. */
export function highlightLines(code: string, linkRefs: boolean): string[] {
	let html = highlightSqlbuild(code);
	if (linkRefs) {
		html = html.replace(
			/(<span class="token [^"]*sb-ref[^"]*">__ref<\/span><span class="token punctuation">\(<\/span>)<span class="token string">"(\w+)"<\/span>/g,
			'$1<button type="button" class="sql-ref" data-ref="$2">"$2"</button>'
		);
	}
	return html.split('\n');
}
