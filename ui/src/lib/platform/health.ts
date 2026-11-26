import type { HealthComponent, ServerHealth } from '$lib/api';

/** How the shell stands: everything working, a part degraded, or nothing that reads the control database works. */
export type ShellHealth = 'live' | 'degraded' | 'unavailable';

/** What the shell says when the server itself doesn't answer. */
export const SERVER_UNREACHABLE = "Can't reach the SQLBuild server";

/** Why nothing works, and since when: the server not answering, or its control database. */
export type Outage = { reason: string; since: string | null };

/** The shell's state from the last health report, or from the server not answering at all. */
export function shellHealth(report: ServerHealth | null, unreachable: Outage | null): ShellHealth | null {
	if (unreachable) return 'unavailable';
	if (!report) return null;
	return report.status === 'healthy' ? 'live' : report.status;
}

/** Why the server can't serve pages, or null while it can. */
export function outageOf(report: ServerHealth | null, unreachable: Outage | null): Outage | null {
	if (unreachable) return unreachable;
	const database = report?.components.find((c) => c.component === 'control_database');
	if (!report || report.status !== 'unavailable' || !database) return null;
	return { reason: database.reason ?? 'The control database is unavailable', since: database.since };
}

/** The parts that aren't healthy while pages still work, worst first. */
export function degradedParts(report: ServerHealth | null): HealthComponent[] {
	if (!report || report.status !== 'degraded') return [];
	return report.components.filter((c) => c.status !== 'healthy');
}

/** The banner's one line: the first degraded part by name and reason, and how many more there are. */
export function degradedLine(parts: HealthComponent[]): string | null {
	const [first, ...rest] = parts;
	if (!first) return null;
	const named = first.reason ? `${first.name}: ${first.reason}` : `${first.name} is degraded`;
	return rest.length ? `${named} (and ${rest.length} more)` : named;
}
