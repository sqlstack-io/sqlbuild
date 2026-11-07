// In-memory stand-in for the server's BUILD endpoints: catalog and lineage,
// checks, the change feed and the overview. Build state is derived the way the
// server derives it: scheduler run nodes plus builds outside the scheduler
// (synced fingerprints), compared against the snapshot change feed.

import { ApiError } from '../client';
import type {
	AttentionItem,
	BuiltDefinition,
	CatalogNode,
	CatalogNodeDetail,
	Capability,
	Check,
	CheckCounts,
	CheckDetail,
	CheckEvaluation,
	CheckEvidence,
	CheckSql,
	CheckOutcome,
	ColumnLineage,
	CostChange,
	GroupStatus,
	MicrobatchCoverage,
	NodeBuild,
	NodeOutcome,
	NodeRunEntry,
	Overview,
	PlatformEvent,
	Run,
	Scope,
	SnapshotChange,
	SnapshotNode,
	StaleReason,
	WarehouseObject
} from '../types';
import { CHECK_SPECS, COLUMN_EDGES, DEPLOYS, EXTERNAL_BUILDS, NODE_SPECS, PYTHON_SOURCE, sourceFreshness, type CheckSpec } from './build-fixtures';
import { compileSql, compileSqlBefore, rawSql, rawSqlBefore } from './build-sql';
import { ancestors, descendants, SNAPSHOT_NODES, snapshotNode } from './operate-graph';
import { TRIGGER_DEFS, triggerId } from './operate';
import type { MockOperate } from './operate-store';
import type { MockPlatformApi } from './platform';
import { invocationId } from './run-detail';

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
const CREDITS_PER_SECOND = 0.0009;
const MODEL_KINDS = new Set(['view', 'table', 'incremental']);

export type BuildContext = {
	now: number;
	can: (capability: Capability) => boolean;
	adapter: (scope: Scope) => string;
	reply: <T>(value: T) => Promise<T>;
	operate: MockOperate;
	platform: MockPlatformApi;
	platformEvents: () => PlatformEvent[];
};

type NodeState = {
	sn: SnapshotNode;
	entries: NodeRunEntry[];
	lastBuild: NodeBuild | null;
	lastOutcome: NodeOutcome | null;
	stale: StaleReason[];
};

type Deploy = (typeof DEPLOYS)[number] & { publishedAt: number };

function hash(text: string): number {
	let h = 2166136261;
	for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 16777619);
	return h >>> 0;
}

const iso = (ms: number) => new Date(ms).toISOString();
const fingerprintOf = (key: string, runId: string) => hash(`${key}/${runId}`).toString(16).padStart(8, '0') + hash(`${runId}/${key}`).toString(16).padStart(8, '0');
const relationOf = (key: string) => {
	const schema = NODE_SPECS[key]?.schema;
	return schema ? { database: 'WAFFLE_SHOP', schema, name: key.toUpperCase() } : null;
};

/** Mock implementation of the BUILD endpoints, delegated to by `MockApiClient`. */
export class MockBuild {
	readonly #ctx: BuildContext;
	readonly #freshness: ReturnType<typeof sourceFreshness>;
	readonly #deploys: Deploy[];

	constructor(ctx: BuildContext) {
		this.#ctx = ctx;
		this.#freshness = sourceFreshness(ctx.now);
		this.#deploys = DEPLOYS.map((d) => ({ ...d, publishedAt: ctx.now - d.minutesAgo * MIN }));
	}

	// ---- catalog and lineage ----

	listCatalogNodes(scope: Scope): Promise<CatalogNode[]> {
		const states = this.#states(scope);
		const checks = this.#checks(scope, states);
		return this.#ctx.reply(SNAPSHOT_NODES.map((sn) => this.#catalogNode(states.get(sn.key)!, checks)));
	}

	async getCatalogNode(scope: Scope, key: string): Promise<CatalogNodeDetail | null> {
		const states = this.#states(scope);
		const state = states.get(key);
		if (!state) return this.#ctx.reply(null);
		const { sn, entries } = state;
		const checks = this.#checks(scope, states);
		const spec = NODE_SPECS[key];
		const builtEntries = entries.filter((e) => e.outcome === 'success' || e.outcome === 'warn');
		const latest = entries[0] ?? null;
		const previous = builtEntries.find((e) => e !== builtEntries[0] && e.rows !== null);
		const costed = this.#hasCost(scope);
		const perBuild = (e: NodeRunEntry) => ((e.durationMs ?? 0) / 1000) * CREDITS_PER_SECOND;
		const recent = builtEntries.filter((e) => Date.parse(e.startedAt) >= Date.now() - DAY);
		const maintenance = await this.#ctx.platform.maintenanceHistory(scope);
		const migrations = maintenance.migrations.filter((m) => m.destinationModel === key);
		const raw = rawSql(sn);
		const detail: CatalogNodeDetail = {
			...this.#catalogNode(state, checks),
			downstream: SNAPSHOT_NODES.filter((n) => n.upstream.includes(key)).map((n) => n.key),
			recentOutcomes: this.#recentOutcomes(key, entries),
			lastRows: builtEntries[0]?.rows ?? null,
			rowsDelta: builtEntries[0]?.rows != null && previous?.rows != null ? builtEntries[0].rows - previous.rows : null,
			lastDurationMs: latest?.durationMs ?? null,
			cost: costed && builtEntries.length ? { last24h: round(recent.reduce((s, e) => s + perBuild(e), 0)), perBuild: round(perBuild(builtEntries[0])), unit: 'credits' } : null,
			columns: (spec?.columns ?? []).map((c) => ({ name: c.name, type: c.type, description: c.description, audits: c.audits ?? [] })),
			sql: raw ? { raw, compiled: compileSql(raw) } : null,
			code: PYTHON_SOURCE[key] ?? null,
			builds: raw ? this.#builtDefinitions(sn, builtEntries) : [],
			jobs: this.#ctx.operate.jobsCovering(key),
			runs: entries.slice(0, 30),
			checkResults: checks.filter((c) => c.node === key),
			warehouse: this.#warehouse(state),
			microbatch: this.#microbatch(state),
			history: {
				formerNames: [...new Set(migrations.map((m) => m.originModel ?? m.origin.name.toLowerCase()).filter((name) => name !== key))],
				migrations,
				oldNameViews: maintenance.oldNameViews.filter((v) => v.destinationModel === key)
			}
		};
		return structuredClone(detail);
	}

	getColumnLineage(scope: Scope, key: string): Promise<ColumnLineage | null> {
		if (!snapshotNode(key)) return this.#ctx.reply(null);
		const states = this.#states(scope);
		const keys = new Set([key, ...ancestors(key), ...descendants(key)].filter((k) => NODE_SPECS[k]?.columns.length));
		const models = SNAPSHOT_NODES.filter((n) => keys.has(n.key)).map((n) => ({
			id: n.key,
			name: n.key,
			kind: NODE_SPECS[n.key].materialization ?? n.kind,
			status: lineageStatus(states.get(n.key)!),
			columns: NODE_SPECS[n.key].columns.map((c) => ({ name: c.name, type: c.type.toUpperCase() }))
		}));
		const edges = COLUMN_EDGES.filter(([from, to]) => keys.has(from.split('.')[0]) && keys.has(to.split('.')[0]));
		const sql: Record<string, string> = {};
		const expressions: Record<string, Record<string, string>> = {};
		for (const m of models) {
			const spec = NODE_SPECS[m.id];
			sql[m.id] = rawSql(snapshotNode(m.id)!) ?? `-- ${snapshotNode(m.id)!.kind}: ${m.id}\n${spec.columns.map((c) => c.name).join(',\n')}`;
			expressions[m.id] = Object.fromEntries(spec.columns.map((c) => [c.name, c.expr ?? c.name]));
		}
		return this.#ctx.reply({ models, edges, sql, expressions });
	}

	// ---- checks ----

	listChecks(scope: Scope): Promise<Check[]> {
		return this.#ctx.reply(this.#checks(scope, this.#states(scope)));
	}

	getCheck(scope: Scope, id: string): Promise<CheckDetail | null> {
		const spec = CHECK_SPECS.find((s) => s.id === id);
		if (!spec) return this.#ctx.reply(null);
		const states = this.#states(scope);
		const history = this.#evaluations(spec, states, 40);
		return this.#ctx.reply({ ...this.#check(spec, history), history });
	}

	getCheckSql(scope: Scope, checkId: string): Promise<CheckSql> {
		if (!this.#ctx.can('view.sql')) throw new ApiError('Missing permission: view.sql', 403);
		const spec = CHECK_SPECS.find((s) => s.id === checkId);
		const states = this.#states(scope);
		const latest = spec && this.#evaluations(spec, states, 1)[0];
		if (!spec || !latest) throw new ApiError(`Unknown check ${checkId}`, 404);
		const relation = spec.node ? relationOf(spec.node) : null;
		const executedSql = spec.sql.replace('{relation}', relation ? `"${relation.database}"."${relation.schema}"."${relation.name}"` : '');
		const failed = latest.outcome === 'error' ? (states.get(spec.node ?? 'daily_revenue')?.entries ?? []).find((e) => e.outcome === 'failed') : undefined;
		this.#ctx.platform.recordSqlRead(scope, checkId, `Read the SQL of ${spec.name} (${latest.id})`);
		return this.#ctx.reply({
			evaluationId: latest.id,
			executedSql,
			measurementSql: spec.evaluationMode === 'measurement' ? executedSql : null,
			evidenceSql: spec.evaluationMode === 'violations' ? executedSql : null,
			executionError: failed?.message ?? null,
			evidenceError: null
		});
	}

	getCheckEvidence(scope: Scope, checkId: string, evaluationId: string): Promise<CheckEvidence> {
		if (!this.#ctx.can('view.evidence')) throw new ApiError('Missing permission: view.evidence', 403);
		const spec = CHECK_SPECS.find((s) => s.id === checkId);
		const evaluation = spec && this.#evaluations(spec, this.#states(scope), 40).find((e) => e.id === evaluationId);
		if (!spec || !evaluation) throw new ApiError(`Unknown evaluation ${evaluationId}`, 404);
		const columns = spec.evidenceColumns ?? (spec.column ? [spec.column] : []);
		const count = Math.min(evaluation.evidenceCount ?? 0, 10);
		const rows = Array.from({ length: count }, (_, i) => columns.map((col) => evidenceValue(col, hash(`${evaluationId}/${i}`))));
		this.#ctx.platform.recordEvidenceRead(scope, checkId, `Read ${count} evidence rows of ${spec.name} (${evaluationId})`);
		return this.#ctx.reply({ evaluationId, columns, rows, truncated: evaluation.evidenceTruncated || (evaluation.evidenceCount ?? 0) > count });
	}

	// ---- change feed ----

	listSnapshotChanges(scope: Scope, query: { limit?: number } = {}): Promise<SnapshotChange[]> {
		return this.#ctx.reply(this.#changes(scope).slice(0, query.limit ?? 50));
	}

	// ---- overview ----

	async overview(scope: Scope): Promise<Overview> {
		const states = this.#states(scope);
		const checks = this.#checks(scope, states);
		const nodes = SNAPSHOT_NODES.map((sn) => this.#catalogNode(states.get(sn.key)!, checks));
		const runs = this.#ctx.operate.topLevelRuns(scope);
		const now = Date.now();
		const dayAgo = now - DAY;
		const recent = runs.filter((r) => Date.parse(r.queuedAt) >= dayAgo);
		const weekAgo = now - 7 * DAY;
		const thisWeek = this.#deploys.filter((d) => d.publishedAt >= weekAgo).flatMap((d) => d.nodes);
		const isModelChange = (key: string) => MODEL_KINDS.has(snapshotNode(key)?.kind ?? 'table');

		const changed = nodes.filter((n) => n.stale?.some((s) => s.reason !== 'behind')).map((n) => n.key);
		const behind = nodes.filter((n) => n.stale?.some((s) => s.reason === 'behind')).map((n) => n.key);
		const staleSources = nodes.filter((n) => n.sourceFreshness && n.sourceFreshness.status !== 'pass');

		const attention: AttentionItem[] = [];
		const pick = (r: Run) => ({ id: r.id, jobKey: r.jobKey, kind: r.kind, status: r.status, statusReason: r.statusReason, startedAt: r.startedAt, endedAt: r.endedAt });
		for (const r of runs.filter((r) => r.endedAt && Date.parse(r.endedAt) >= dayAgo && ['failed', 'partial', 'presumed_failed'].includes(r.status)).slice(0, 3)) {
			attention.push({ kind: 'run_problem', run: pick(r) });
		}
		for (const r of runs.filter((r) => r.status === 'unresponsive')) {
			attention.push({ kind: 'run_unresponsive', run: pick(r), lastHeartbeatAt: this.#ctx.operate.lastHeartbeat(r.id) });
		}
		if (changed.length || behind.length) attention.push({ kind: 'stale_models', changed, behind });
		if (staleSources.length) attention.push({ kind: 'stale_sources', sources: staleSources.map((n) => ({ key: n.key, ...n.sourceFreshness! })) });
		for (const event of this.#ctx.platformEvents().filter((e) => e.active)) attention.push({ kind: 'platform_event', event });
		const queue = await this.#ctx.operate.listQueue(scope);
		for (const entry of queue.filter((q) => q.waits.length && Date.parse(q.run.queuedAt) < now - 10 * MIN && !q.run.parentRunId)) {
			attention.push({ kind: 'queue_blocked', runId: entry.run.id, jobKey: entry.run.jobKey, queuedAt: entry.run.queuedAt, waits: entry.waits });
		}
		const triggers = await this.#ctx.operate.listTriggers(scope);
		for (const t of triggers.filter((t) => t.kind === 'calendar_schedule' && t.warning)) {
			attention.push({ kind: 'calendar_low', triggerId: t.id, triggerName: t.name, detail: t.warning! });
		}

		const cost = this.#hasCost(scope) ? recent.reduce((s, r) => s + (r.cost?.value ?? 0), 0) : null;
		const groups = new Map<string, GroupStatus>();
		for (const n of nodes) {
			if (n.kind === 'source') continue;
			const group = n.group ?? '';
			const g = groups.get(group) ?? { group, total: 0, current: 0, changed: 0, behind: 0, failed: 0 };
			g.total += 1;
			if (n.lastOutcome === 'failed') g.failed += 1;
			const stale = n.stale ?? [];
			if (stale.some((s) => s.reason !== 'behind')) g.changed += 1;
			if (stale.some((s) => s.reason === 'behind')) g.behind += 1;
			if (!stale.length && n.lastOutcome !== 'failed' && n.lastBuild) g.current += 1;
			groups.set(group, g);
		}
		const latest = runs.find((r) => r.endedAt);
		return this.#ctx.reply({
			metrics: {
				models: {
					total: nodes.filter((n) => MODEL_KINDS.has(n.kind)).length,
					addedThisWeek: thisWeek.filter((c) => c.op === 'added' && isModelChange(c.key)).length,
					removedThisWeek: thisWeek.filter((c) => c.op === 'removed').length
				},
				runs24h: {
					total: recent.length,
					success: recent.filter((r) => r.status === 'success').length,
					warn: recent.filter((r) => r.status === 'warn').length,
					partial: recent.filter((r) => r.status === 'partial').length,
					failed: recent.filter((r) => r.status === 'failed' || r.status === 'presumed_failed').length,
					active: recent.filter((r) => ['queued', 'launched', 'running', 'unresponsive'].includes(r.status)).length
				},
				checks: countChecks(checks),
				stale: { nodes: new Set([...changed, ...behind]).size, changed: changed.length, behind: behind.length, sources: staleSources.length },
				// The mock holds one day of costed runs; the server's baseline is the rolling average of the same window.
				cost: cost === null ? null : { last24h: round(cost), baseline: round(cost * 0.78), baselineDays: 7, unit: 'credits' }
			},
			attention,
			groups: [...groups.values()],
			recentChanges: this.#changes(scope).slice(0, 4),
			latestRun: latest ? { id: latest.id, status: latest.status, endedAt: latest.endedAt } : null
		});
	}

	/** Nodes in the latest snapshot, for the navigation count. */
	nodeCount(): number {
		return SNAPSHOT_NODES.length;
	}

	failingCheckCount(scope: Scope): number {
		const counts = countChecks(this.#checks(scope, this.#states(scope)));
		return counts.failing + counts.warning;
	}

	// ---- internals: build state ----

	#hasCost(scope: Scope): boolean {
		return this.#ctx.adapter(scope) === 'snowflake';
	}

	#states(scope: Scope): Map<string, NodeState> {
		const states = new Map<string, NodeState>();
		for (const sn of SNAPSHOT_NODES) {
			const entries = this.#entries(scope, sn.key);
			const built = entries.find((e) => e.outcome === 'success' || e.outcome === 'warn');
			const lastBuild: NodeBuild | null = built
				? { fingerprintRunId: built.fingerprintRunId!, runId: built.runId, jobKey: built.jobKey, builtAt: built.startedAt, fingerprint: fingerprintOf(sn.key, built.fingerprintRunId!) }
				: null;
			states.set(sn.key, { sn, entries, lastBuild, lastOutcome: entries.find((e) => e.outcome !== 'skipped')?.outcome ?? null, stale: [] });
		}
		for (const state of states.values()) state.stale = this.#stale(state, states);
		return states;
	}

	#entries(scope: Scope, key: string): NodeRunEntry[] {
		const scheduler: NodeRunEntry[] = this.#ctx.operate
			.nodeHistory(scope, key)
			.filter(({ node }) => node.status === 'success' || node.status === 'warn' || node.status === 'failed' || node.status === 'skipped')
			.map(({ run, node }) => ({
				runId: run.id,
				fingerprintRunId: node.status === 'success' || node.status === 'warn' ? invocationId(run) : null,
				jobKey: run.jobKey,
				outcome: node.status as NodeOutcome,
				startedAt: node.startedAt ?? run.queuedAt,
				durationMs: node.startedAt && node.endedAt ? Date.parse(node.endedAt) - Date.parse(node.startedAt) : null,
				rows: node.rowsAffected,
				message: node.errorMessage ?? (node.skippedBecauseOf ? `Skipped: upstream ${node.skippedBecauseOf.nodeKey} failed` : null)
			}));
		const external: NodeRunEntry[] = EXTERNAL_BUILDS.filter((b) => b.node === key).map((b) => ({
			runId: null,
			fingerprintRunId: b.fingerprintRunId,
			jobKey: null,
			outcome: b.outcome,
			startedAt: iso(this.#ctx.now - b.minutesAgo * MIN),
			durationMs: b.durationMs,
			rows: b.rows,
			message: null
		}));
		return [...scheduler, ...external].sort((a, b) => b.startedAt.localeCompare(a.startedAt));
	}

	#stale(state: NodeState, states: Map<string, NodeState>): StaleReason[] {
		const { sn, lastBuild } = state;
		if (sn.kind === 'source') return [];
		const builtAt = lastBuild ? Date.parse(lastBuild.builtAt) : -Infinity;
		const reasons: StaleReason[] = [];
		const own = this.#deploys.find((d) => d.publishedAt > builtAt && d.nodes.some((n) => n.key === sn.key && n.op !== 'removed'));
		if (own) {
			const change = own.nodes.find((n) => n.key === sn.key)!;
			reasons.push({
				reason: 'changed',
				change: change.compiledOnly ? 'compiled' : 'definition',
				detail: lastBuild ? `${change.detail} in generation ${own.generation}` : 'Never built in this target',
				since: iso(own.publishedAt)
			});
		} else if (!lastBuild) {
			reasons.push({ reason: 'changed', change: 'definition', detail: 'Never built in this target', since: iso(this.#deploys[this.#deploys.length - 1].publishedAt) });
		}
		const changedUpstream = [...ancestors(sn.key)].filter((up) => this.#deploys.some((d) => d.publishedAt > builtAt && d.nodes.some((n) => n.key === up && n.op !== 'removed')));
		if (changedUpstream.length && lastBuild) {
			const since = Math.min(...this.#deploys.filter((d) => d.publishedAt > builtAt && d.nodes.some((n) => changedUpstream.includes(n.key))).map((d) => d.publishedAt));
			reasons.push({ reason: 'changed_upstream', upstream: changedUpstream, since: iso(since) });
		}
		// Views read their upstream live, so only materialized nodes fall behind.
		if (lastBuild && sn.kind !== 'view') {
			const newer = sn.upstream.map((up) => ({ up, at: this.#dataTime(up, states) })).filter((u) => u.at > builtAt);
			if (newer.length) {
				reasons.push({ reason: 'behind', upstream: newer.map((u) => u.up), detail: 'Upstream has newer data than the last build', since: iso(Math.min(...newer.map((u) => u.at))) });
			}
		}
		return reasons;
	}

	/** When a node's data last changed: a source's newest load, a view's upstream, or a build. */
	#dataTime(key: string, states: Map<string, NodeState>): number {
		const state = states.get(key);
		if (!state) return -Infinity;
		if (state.sn.kind === 'source') {
			const loaded = this.#freshness[key]?.loadedAt;
			return loaded ? Date.parse(loaded) : -Infinity;
		}
		if (state.sn.kind === 'view') return Math.max(-Infinity, ...state.sn.upstream.map((up) => this.#dataTime(up, states)));
		return state.lastBuild ? Date.parse(state.lastBuild.builtAt) : -Infinity;
	}

	#catalogNode(state: NodeState, checks: Check[]): CatalogNode & Pick<CatalogNodeDetail, 'stale' | 'checks'> {
		const { sn, lastBuild, lastOutcome, stale } = state;
		const spec = NODE_SPECS[sn.key];
		return {
			key: sn.key,
			kind: sn.kind,
			materialization: spec?.materialization ?? null,
			group: sn.group,
			tags: [...sn.tags],
			owner: [...(spec?.owner ?? [])],
			description: spec?.description ?? '',
			path: sn.path,
			upstream: [...sn.upstream],
			relation: relationOf(sn.key),
			columnCount: spec?.columns.length ?? 0,
			lastBuild,
			lastOutcome,
			stale: structuredClone(stale),
			sourceFreshness: this.#freshness[sn.key] ?? null,
			checks: countChecks(checks.filter((c) => c.node === sn.key))
		};
	}

	#recentOutcomes(key: string, entries: NodeRunEntry[]): NodeOutcome[] {
		const real = entries.map((e) => e.outcome).reverse();
		if (!real.length) return [];
		const pad = Array.from({ length: Math.max(0, 40 - real.length) }, (_, i) => {
			const h = hash(`${key}/${i}`) % 23;
			return (h === 0 ? 'failed' : h < 3 ? 'warn' : 'success') as NodeOutcome;
		});
		return [...pad, ...real].slice(-40);
	}

	#builtDefinitions(sn: SnapshotNode, built: NodeRunEntry[]): BuiltDefinition[] {
		return built.slice(0, 4).map((e) => {
			const at = Date.parse(e.startedAt);
			const generation = this.#generationAt(at);
			const raw = rawSqlBefore(sn, generation + 1) ?? '';
			return {
				build: { fingerprintRunId: e.fingerprintRunId!, runId: e.runId, jobKey: e.jobKey, builtAt: e.startedAt, fingerprint: fingerprintOf(sn.key, e.fingerprintRunId!) },
				raw,
				compiled: compileSqlBefore(raw, generation + 1)
			};
		});
	}

	/** The generation in force at `at`. */
	#generationAt(at: number): number {
		return this.#deploys.find((d) => d.publishedAt <= at)?.generation ?? this.#deploys[this.#deploys.length - 1].generation - 1;
	}

	#warehouse(state: NodeState): WarehouseObject | null {
		const { sn, lastBuild } = state;
		const relation = relationOf(sn.key);
		const spec = NODE_SPECS[sn.key];
		if (!relation || !spec) return null;
		const now = this.#ctx.now;
		const builtAt = lastBuild ? Date.parse(lastBuild.builtAt) : null;
		const missing = new Set(
			this.#deploys
				.filter((d) => builtAt === null || d.publishedAt > builtAt)
				.flatMap((d) => d.nodes.filter((n) => n.key === sn.key).flatMap((n) => n.columns.filter((c) => c.op === 'added').map((c) => c.name)))
		);
		const columns: WarehouseObject['columns'] = spec.columns.map((c) => ({
			name: c.name,
			declaredType: c.type,
			actualType: missing.has(c.name) ? null : warehouseType(c.type),
			state: missing.has(c.name) ? 'missing' : 'match'
		}));
		if (sn.kind === 'source') columns.push({ name: '_synced_at', declaredType: null, actualType: 'TIMESTAMP_NTZ(9)', state: 'undeclared' });
		const isView = sn.kind === 'view';
		const base = 800 + (hash(sn.key) % 40_000);
		const stats = isView
			? []
			: Array.from({ length: 28 }, (_, i) => {
					const rows = Math.round(base * (0.82 + i * 0.0068));
					return { at: iso(now - (27 - i) * 6 * HOUR), rows, bytes: rows * (60 + spec.columns.length * 14) };
				});
		return {
			relation,
			objectType: isView ? 'view' : 'table',
			createdAt: iso(now - (120 + (hash(sn.key) % 200)) * DAY),
			lastAlteredAt: lastBuild?.builtAt ?? iso(now - 30 * DAY),
			rows: stats.at(-1)?.rows ?? null,
			bytes: stats.at(-1)?.bytes ?? null,
			columns,
			stats,
			syncedAt: iso(now - 4 * MIN)
		};
	}

	#microbatch(state: NodeState): MicrobatchCoverage | null {
		const spec = NODE_SPECS[state.sn.key];
		if (!spec?.microbatch) return null;
		const hourly = spec.microbatch.batchSize === '1h';
		const size = hourly ? HOUR : DAY;
		const count = hourly ? 48 : 21;
		const end = Math.floor(this.#ctx.now / size) * size;
		const runs = state.entries.filter((e) => e.runId).map((e) => e.runId);
		const gapAt = state.sn.key === 'hourly_order_activity' ? count - 9 : -1;
		const replayFrom = state.sn.key === 'hourly_activity_ctx' ? count - 3 : -1;
		const batches = Array.from({ length: count }, (_, i) => {
			const start = end - (count - i) * size;
			const state: MicrobatchCoverage['batches'][number]['state'] = i === gapAt ? 'gap' : replayFrom >= 0 && i >= replayFrom ? 'replay_required' : 'complete';
			return { start: iso(start), end: iso(start + size), state, runId: state === 'complete' ? (runs[(count - 1 - i) % Math.max(1, runs.length)] ?? null) : null };
		});
		const firstOpen = batches.findIndex((b) => b.state !== 'complete');
		return {
			cursorColumn: spec.microbatch.cursorColumn,
			cursorType: 'timestamp',
			batchSize: spec.microbatch.batchSize,
			earliest: iso(end - 90 * DAY),
			contiguousFrontier: firstOpen >= 0 ? batches[firstOpen].start : batches[batches.length - 1].end,
			knownGaps: batches.filter((b) => b.state === 'gap').length,
			batches,
			batchesTruncated: true
		};
	}

	// ---- internals: checks ----

	#checks(scope: Scope, states: Map<string, NodeState>): Check[] {
		void scope;
		return CHECK_SPECS.map((spec) => this.#check(spec, this.#evaluations(spec, states, 20)));
	}

	#check(spec: CheckSpec, history: CheckEvaluation[]): Check {
		return {
			id: spec.id,
			name: spec.name,
			definition: spec.definition,
			kind: spec.kind,
			attachment: spec.attachment,
			node: spec.node,
			column: spec.column,
			severity: spec.severity,
			runScopePhase: spec.runScopePhase,
			evaluationMode: spec.evaluationMode,
			thresholds: structuredClone(spec.thresholds),
			description: spec.description,
			latest: history[0] ?? null,
			recentOutcomes: history.slice(0, 20).map((e) => e.outcome).reverse()
		};
	}

	/** Evaluations newest first: one per build of the attached node, then older synced results. */
	#evaluations(spec: CheckSpec, states: Map<string, NodeState>, limit: number): CheckEvaluation[] {
		const anchor = spec.node ?? 'daily_revenue';
		const entries = (states.get(anchor)?.entries ?? []).filter((e) => e.outcome !== 'skipped');
		const measured = (i: number) => (spec.history.measuredBase !== undefined ? round(spec.history.measuredBase * (0.85 + (hash(`${spec.id}/${i}`) % 30) / 100), 3) : null);
		const warnLevel = spec.thresholds.find((t) => t.level === 'warn');
		const breach = warnLevel && typeof warnLevel.value === 'number' ? round(warnLevel.value * (warnLevel.operator === 'below' ? 0.9 : 1.4), 3) : null;
		const isViolations = spec.evaluationMode === 'violations';
		type Extra = { runId?: string | null; evaluatedAt: string; violations?: number; measured?: number };
		const at = (seed: string, index: number, outcome: CheckOutcome, extra: Extra): CheckEvaluation => {
			const counted = outcome === 'pass' || outcome === 'warn' || outcome === 'fail';
			const violations = !isViolations || !counted ? null : outcome === 'pass' ? 0 : (extra.violations ?? 1 + (hash(seed) % 5));
			return {
				id: `ar-${hash(`${spec.id}/${seed}`).toString(16)}`,
				runId: extra.runId ?? null,
				fingerprintRunId: seed,
				evaluatedAt: extra.evaluatedAt,
				outcome,
				violationCount: violations,
				measuredValue: isViolations || !counted ? null : (extra.measured ?? (outcome === 'pass' ? measured(index) : breach)),
				evidenceCount: violations,
				evidenceTruncated: (violations ?? 0) > 10
			};
		};
		// Audits run inside the build, so the node's outcome bounds the check's.
		const out: CheckEvaluation[] = entries.map((e, i) => {
			const seed = e.fingerprintRunId ?? invocationIdLike(e.runId ?? spec.id);
			const base = { runId: e.runId, evaluatedAt: e.startedAt };
			if (e.outcome === 'failed') return spec.errorOnFailure ? at(seed, i, 'error', base) : at(seed, i, 'skipped', base);
			const latest = i === 0 ? { violations: spec.latest.violations, measured: spec.latest.measured } : {};
			if (e.outcome === 'warn' && spec.latest.outcome === 'warn') return at(seed, i, 'warn', { ...base, ...latest, violations: i === 0 ? spec.latest.violations : undefined });
			return at(seed, i, 'pass', { ...base, ...(spec.latest.outcome === 'pass' ? latest : {}) });
		});
		// Older results from builds the scheduler did not run, every four hours.
		const oldest = out.length ? Date.parse(out[out.length - 1].evaluatedAt) : this.#ctx.now - 30 * MIN;
		for (let i = out.length; i < limit; i++) {
			const seed = synthRunId(spec.id, i);
			const fail = spec.history.failEvery && i % spec.history.failEvery === spec.history.failEvery - 1;
			const warn = spec.history.warnEvery && i % spec.history.warnEvery === spec.history.warnEvery - 1;
			const outcome: CheckOutcome = !out.length && i === 0 ? spec.latest.outcome : fail ? (spec.severity === 'error' ? 'fail' : 'warn') : warn ? 'warn' : 'pass';
			out.push(at(seed, i, outcome, { evaluatedAt: iso(oldest - (i - entries.length + 1) * 4 * HOUR), ...(i === 0 ? { violations: spec.latest.violations, measured: spec.latest.measured } : {}) }));
		}
		return out.slice(0, limit);
	}

	// ---- internals: change feed ----

	#changes(scope: Scope): SnapshotChange[] {
		const runs = this.#ctx.operate.topLevelRuns(scope);
		const costed = this.#hasCost(scope);
		const windowRuns = (i: number) => {
			const from = this.#deploys[i].publishedAt;
			const until = i > 0 ? this.#deploys[i - 1].publishedAt : Infinity;
			return runs.filter((r) => Date.parse(r.queuedAt) >= from && Date.parse(r.queuedAt) < until);
		};
		return this.#deploys.map((d, i) => {
			const after = windowRuns(i);
			const before = i + 1 < this.#deploys.length ? windowRuns(i + 1) : [];
			let costChanges: CostChange[] | null = null;
			if (costed) {
				const avg = (rs: Run[], job: string) => {
					const costs = rs.filter((r) => r.jobKey === job && r.cost).map((r) => r.cost!.value);
					return costs.length ? costs.reduce((a, b) => a + b, 0) / costs.length : null;
				};
				costChanges = [...new Set(after.map((r) => r.jobKey).filter((k): k is string => !!k))].flatMap((job) => {
					const b = avg(before, job);
					const a = avg(after, job);
					return a !== null && b !== null && Math.abs(a - b) / b >= 0.1 ? [{ jobKey: job, before: round(b), after: round(a), unit: 'credits' as const, runsAfter: after.filter((r) => r.jobKey === job && r.cost).length }] : [];
				});
			}
			return {
				snapshotId: `snap-${d.gitRevision}`,
				generation: d.generation,
				previousGeneration: this.#deploys[i + 1]?.generation ?? null,
				gitRevision: d.gitRevision,
				gitDirty: d.gitDirty,
				gitAuthor: d.gitAuthor,
				publishedAt: iso(d.publishedAt),
				publishedBy: d.publishedBy,
				nodes: d.nodes.map((n) => ({ ...n, kind: snapshotNode(n.key)?.kind ?? null, columns: [...n.columns] })),
				orchestration: d.orchestration.map((o) => ({ ...o, triggerId: (o.kind === 'trigger' || o.kind === 'sensor') && TRIGGER_DEFS.some((t) => t.name === o.key) ? triggerId(o.key) : null })),
				roles: structuredClone(d.roles),
				environment: d.environment ? { ...d.environment } : null,
				followingRuns: [...after].reverse().slice(0, 6).map((r) => ({ id: r.id, jobKey: r.jobKey, kind: r.kind, status: r.status, startedAt: r.startedAt, queuedAt: r.queuedAt })),
				costChanges
			};
		});
	}
}

function round(value: number, places = 4): number {
	const f = 10 ** places;
	return Math.round(value * f) / f;
}

function countChecks(checks: Check[]): CheckCounts {
	const of = (...outcomes: CheckOutcome[]) => checks.filter((c) => c.latest && outcomes.includes(c.latest.outcome)).length;
	return { total: checks.length, passing: of('pass'), warning: of('warn'), failing: of('fail', 'error') };
}

function lineageStatus(state: NodeState): 'ok' | 'fail' | 'warn' | 'stale' | 'src' {
	if (state.sn.kind === 'source') return 'src';
	if (state.lastOutcome === 'failed') return 'fail';
	if (state.stale.length) return 'stale';
	return state.lastOutcome === 'warn' ? 'warn' : 'ok';
}

function warehouseType(declared: string): string {
	const t = declared.toLowerCase();
	if (t === 'bigint' || t === 'integer') return 'NUMBER(38,0)';
	if (t.startsWith('decimal')) return t.replace('decimal', 'NUMBER').toUpperCase();
	if (t === 'varchar') return 'VARCHAR(16777216)';
	if (t === 'timestamp') return 'TIMESTAMP_NTZ(9)';
	if (t === 'double') return 'FLOAT';
	return t.toUpperCase();
}

function evidenceValue(column: string, seed: number): string | number | null {
	if (column.endsWith('_id') || column === 'customer_id') return 100_000 + (seed % 90_000);
	if (column === 'n') return 2 + (seed % 3);
	if (column.endsWith('_cents')) return seed % 2 ? -(seed % 900) : seed % 5000;
	if (column === 'payment_method') return ['card', 'cash', 'gift_card', null][seed % 4];
	if (column.endsWith('_date')) return iso(Date.UTC(2026, 9, 1 + (seed % 28))).slice(0, 10);
	return null;
}

function synthRunId(seed: string, i: number): string {
	const h = (n: number) => hash(`${seed}/${i}/${n}`).toString(16).padStart(8, '0');
	return `${h(1)}-${h(2).slice(0, 4)}-4${h(3).slice(0, 3)}-a${h(4).slice(0, 3)}-${h(5)}${h(6).slice(0, 4)}`;
}

function invocationIdLike(seed: string): string {
	return synthRunId(seed, 0);
}
