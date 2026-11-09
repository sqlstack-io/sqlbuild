import type { components } from '$control/schema.d.ts';
import type { JanitorEvent, MaintenanceHistory, MigrationEvent, OldNameEventType, OldNameView, RelationName } from './types';

type Wire = components['schemas'];

const relation = (database: string | null | undefined, schema: string | null | undefined, name: string): RelationName => ({ database: database ?? null, schema: schema ?? '', name });

function toMigration(w: Wire['WarehouseMigrationRecord']): MigrationEvent {
	return {
		eventId: w.event_id,
		targetName: w.target_name ?? '',
		originModel: w.origin_model ?? null,
		origin: relation(w.origin_database, w.origin_schema, w.origin_name),
		destinationModel: w.destination_model,
		destination: relation(w.destination_database, w.destination_schema, w.destination_name),
		discovery: w.discovery as MigrationEvent['discovery'],
		decision: w.decision as MigrationEvent['decision'],
		runId: w.run_id,
		createdAt: w.created_at
	};
}

/** Stored as compact JSON of `{old: new}`. */
function columnAliases(text: string | null | undefined): OldNameView['columnAliases'] {
	return text ? Object.entries(JSON.parse(text) as Record<string, string>).map(([old, renamed]) => ({ old, new: renamed })) : [];
}

/** Stored as a JSON list of the grant statements copied to the view. */
const grantCount = (text: string | null | undefined): number | null => (text ? (JSON.parse(text) as string[]).length : null);

/** One view per move, from its append-only steps (oldest first); each fact comes from the newest step that records it. */
function toOldNameViews(rows: Wire['WarehouseOldNameRecord'][], migrationOrder: string[]): OldNameView[] {
	const steps = new Map<string, Wire['WarehouseOldNameRecord'][]>();
	for (const row of rows) steps.set(row.migration_event_id, [...(steps.get(row.migration_event_id) ?? []), row]);
	const rank = (id: string) => (migrationOrder.includes(id) ? migrationOrder.indexOf(id) : migrationOrder.length);
	return [...steps.entries()]
		.sort(([a], [b]) => rank(a) - rank(b))
		.map(([migrationEventId, recorded]) => {
			const first = recorded[0];
			const newest = <K extends keyof Wire['WarehouseOldNameRecord']>(key: K) => [...recorded].reverse().find((r) => r[key] != null)?.[key] ?? null;
			return {
				migrationEventId,
				destinationModel: first.destination_model,
				old: relation(first.old_database, first.old_schema, first.old_name),
				new: relation(first.new_database, first.new_schema, first.new_name),
				retention: (newest('view_retention') as string | null) ?? '',
				archiveName: newest('archive_name') as string | null,
				columnAliases: columnAliases(newest('column_aliases') as string | null),
				grantsCopied: grantCount(newest('grants_copied') as string | null),
				expiresAt: newest('expires_at') as string | null,
				dropReason: newest('drop_reason') as string | null,
				steps: recorded.map((r) => ({ eventType: r.event_type as OldNameEventType, at: r.created_at, runId: r.run_id }))
			};
		});
}

function toJanitorEvent(w: Wire['WarehouseJanitorRecord']): JanitorEvent {
	return {
		eventId: w.event_id,
		eventType: w.event_type as JanitorEvent['eventType'],
		occurredAt: w.occurred_at,
		runId: w.run_id,
		relationSchema: w.relation_schema,
		relationType: (w.relation_type ?? null) as JanitorEvent['relationType'],
		originalQualifiedName: w.original_qualified_name ?? null,
		archiveQualifiedName: w.archive_qualified_name,
		archivedAt: w.archived_at
	};
}

/** Synced maintenance history; the server lists the newest 100 moves and clean-ups. */
export function toMaintenanceHistory(wire: Wire['MaintenanceHistory']): MaintenanceHistory {
	const migrations = wire.migrations.map(toMigration);
	return {
		migrations,
		oldNameViews: toOldNameViews(wire.old_name_views, migrations.map((m) => m.eventId)),
		janitorEvents: wire.janitor_events.map(toJanitorEvent)
	};
}
