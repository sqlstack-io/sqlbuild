import type { Account, AssignmentSource } from '$lib/api';
import { assignmentsAt } from '$lib/platform/access';
import { createAccessDirectory } from './access-directory.svelte';

/** The People or Service accounts table: accounts with their roles, filtered. Must be created during component initialisation. */
export function createAccountTable(kind: Account['kind']) {
	const directory = createAccessDirectory();

	let query = $state('');
	/** Any of these roles; none means any role. */
	let role = $state<string[]>([]);
	let locationId = $state('');
	/** Any of these sources; none means any source. */
	let source = $state<AssignmentSource[]>([]);
	let riskyOnly = $state(false);

	const people = $derived((directory.accounts.data ?? []).filter((a) => a.kind === kind));
	const roleNames = $derived([...new Set((directory.assignments.data ?? []).filter((a) => !a.stale).map((a) => a.role))].sort());

	function relevant(name: string) {
		const all = directory.assignmentsOf(name);
		return locationId ? assignmentsAt(all, locationId) : all;
	}

	const shown = $derived.by(() => {
		const q = query.trim().toLowerCase();
		return people.filter((p) => {
			const mine = relevant(p.name);
			if (q && !`${p.name} ${p.displayName} ${p.email ?? ''} ${mine.map((m) => m.role).join(' ')}`.toLowerCase().includes(q)) return false;
			if (role.length && !mine.some((m) => role.includes(m.role) && !m.stale)) return false;
			if (locationId && !mine.length) return false;
			if (source.length && !mine.some((m) => source.includes(m.source))) return false;
			if (riskyOnly && !directory.riskyOf(p.name).length) return false;
			return true;
		});
	});

	return {
		directory,
		/** Every account of the kind, before filters. */
		get accounts() {
			return people;
		},
		get shown() {
			return shown;
		},
		get roleNames() {
			return roleNames;
		},
		relevant,
		get query() {
			return query;
		},
		set query(v: string) {
			query = v;
		},
		get role() {
			return role;
		},
		set role(v: string[]) {
			role = v;
		},
		get locationId() {
			return locationId;
		},
		set locationId(v: string) {
			locationId = v;
		},
		get source() {
			return source;
		},
		set source(v: AssignmentSource[]) {
			source = v;
		},
		get riskyOnly() {
			return riskyOnly;
		},
		set riskyOnly(v: boolean) {
			riskyOnly = v;
		}
	};
}
