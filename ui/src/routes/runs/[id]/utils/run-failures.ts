// What made a run fail, for the summary under its title: failed checks and failed nodes, each with its
// warehouse error code and a short cause. Never the worker's own end line. Pure.

import type { RunEvent, RunNode, RunStatement } from '$lib/api';

export type RunFailure = {
	id: string;
	kind: 'check' | 'node';
	/** The audit's or the node's name. */
	name: string;
	/** Where it ran: a node's name, or `after the build` for an end-of-build audit. */
	where: string | null;
	/** The warehouse's SQLSTATE, such as `42S02`, else the node's own error code. */
	code: string | null;
	/** The error text as the warehouse or sqb gave it; null until it has been read (it needs view.evidence). */
	message: string | null;
	/** The cause in a few words, when the code says it. */
	cause: string | null;
	statementId: string | null;
	nodeKey: string | null;
	/** The Checks page's ID for an audit, when the server reported it. */
	checkId: string | null;
	/** The log filter that shows this failure. */
	logQuery: string;
};

/** Error text the page has read, behind view.evidence: a failed node's by node, a failed audit's by its result event. */
export type FailureTexts = { node: (n: RunNode) => string | null; audit: (eventId: string) => string | null };

/** Short causes for the SQLSTATEs that say what went wrong; `{target}` is the run's target. */
const CAUSES: Record<string, string> = {
	'42S02': 'input not built in {target}',
	'42501': 'no access to an input',
	'42000': 'SQL error',
	'22012': 'division by zero',
	'57014': 'query cancelled'
};

/** A warehouse error's SQLSTATE: the five characters in brackets, as in `002003 (42S02): …`. */
export function sqlState(text: string | null | undefined): string | null {
	return text?.match(/\(([0-9A-Z]{5})\)/)?.[1] ?? null;
}

/** An error's last non-empty line: the warehouse's own sentence, without its code prefix. */
export function errorLine(text: string): string {
	return text.split('\n').filter((l) => l.trim()).pop() ?? text;
}

/** The relations a query reads, as lower-case names: `"DB"."SCHEMA"."NAME"` or `db.schema.name` after FROM or JOIN. */
export function relationsRead(sql: string): string[] {
	const out: string[] = [];
	for (const m of sql.matchAll(/\b(?:FROM|JOIN)\s+((?:"[^"]+"|[\w$]+)(?:\.(?:"[^"]+"|[\w$]+)){0,2})/gi)) {
		const name = m[1].split('.').pop()!.replace(/"/g, '').toLowerCase();
		if (!out.includes(name)) out.push(name);
	}
	return out;
}

/** Failed audits, then failed nodes, in the order they ended; error text where it has been read. */
export function runFailures(nodes: readonly RunNode[], events: readonly RunEvent[], statements: readonly RunStatement[], target: string, texts: FailureTexts): RunFailure[] {
	const causeOf = (code: string | null) => (code && CAUSES[code] ? CAUSES[code].replace('{target}', target) : null);
	const checks = events
		.filter((e) => e.type === 'audit_completed' && e.level === 'error' && e.check?.outcome === 'failed')
		.map((e): RunFailure & { at: string } => {
			const message = texts.audit(e.id);
			const code = sqlState(message);
			return {
				id: e.id,
				at: e.occurredAt,
				kind: 'check',
				name: e.check!.name,
				where: e.nodeKey ?? 'after the build',
				code,
				message,
				cause: causeOf(code) ?? (e.check!.rows ? `${e.check!.rows.toLocaleString('en-GB')} failing rows` : null),
				statementId: e.statementId,
				nodeKey: e.nodeKey,
				checkId: e.check!.bindingKey,
				logQuery: `kind:audit outcome:failed`
			};
		});
	const failedNodes = nodes
		.filter((n) => n.status === 'failed')
		.map((n): RunFailure & { at: string } => {
			const message = n.errorDetail ?? texts.node(n) ?? n.errorMessage;
			const st = statements.find((s) => s.nodeKey === n.nodeKey && s.runId === n.runId && s.status === 'failed');
			const code = sqlState(message) ?? n.errorCode;
			return {
				id: `${n.runId}/${n.nodeKey}`,
				at: n.endedAt ?? '',
				kind: 'node',
				name: n.nodeKey,
				where: null,
				code,
				message,
				cause: causeOf(code),
				statementId: st?.id ?? null,
				nodeKey: n.nodeKey,
				checkId: null,
				logQuery: `node:${n.nodeKey}`
			};
		});
	return [...checks, ...failedNodes].sort((a, b) => a.at.localeCompare(b.at)).map(({ at: _at, ...f }) => f);
}

/** A failure's one line: `Failed: audit orders_match_payments after the build · input not built in prod`. */
export function failureLine(f: RunFailure): string {
	const what = f.kind === 'check' ? `audit ${f.name}${f.where ? ` ${f.where === 'after the build' ? f.where : `on ${f.where}`}` : ''}` : f.name;
	const why = f.cause ?? (f.message ? errorLine(f.message) : null);
	return why ? `Failed: ${what} · ${why}` : `Failed: ${what}`;
}
