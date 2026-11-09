import CheckCircleIcon from '@lucide/svelte/icons/circle-check';
import ClockIcon from '@lucide/svelte/icons/clock-3';
import DatabaseIcon from '@lucide/svelte/icons/database';
import GitBranchIcon from '@lucide/svelte/icons/git-branch';
import GitCompareIcon from '@lucide/svelte/icons/git-compare';
import UsersIcon from '@lucide/svelte/icons/users';
import LayoutDashboardIcon from '@lucide/svelte/icons/layout-dashboard';
import ListIcon from '@lucide/svelte/icons/list';
import MapPinIcon from '@lucide/svelte/icons/map-pin';
import PlayIcon from '@lucide/svelte/icons/play';
import PlugIcon from '@lucide/svelte/icons/plug';
import PuzzleIcon from '@lucide/svelte/icons/puzzle';
import ScrollTextIcon from '@lucide/svelte/icons/scroll-text';
import ShieldIcon from '@lucide/svelte/icons/shield';
import WrenchIcon from '@lucide/svelte/icons/wrench';
import type { Component } from 'svelte';
import { ADMIN_CAPABILITIES, hasAnyCapability, type ApiClient, type Capability, type NavCounts } from '$lib/api';

export type NavBadge = { value: (counts: NavCounts) => number; tone: 'muted' | 'warning' | 'error' | 'live' };
/**
 * `loads` is the client method the page cannot work without; the entry hides when the server lacks it.
 * `wholeTarget` pages list the selected target, which needs view on every node in it.
 */
export type NavItem = { label: string; href: string; icon: Component; loads: keyof ApiClient; badge?: NavBadge; requires?: readonly Capability[]; wholeTarget?: true };
export type NavGroup = { section: string | null; items: NavItem[]; requires?: readonly Capability[] };

export const NAV: readonly NavGroup[] = [
	{ section: null, items: [{ label: 'Overview', href: '/', icon: LayoutDashboardIcon, loads: 'overview', wholeTarget: true }] },
	{
		section: 'Build',
		items: [
			{ label: 'Lineage', href: '/lineage', icon: GitBranchIcon, loads: 'listCatalogNodes', wholeTarget: true },
			{ label: 'Catalog', href: '/catalog', icon: DatabaseIcon, loads: 'listCatalogNodes', badge: { value: (c) => c.catalogNodes, tone: 'muted' }, wholeTarget: true },
			{ label: 'Checks', href: '/checks', icon: CheckCircleIcon, loads: 'listChecks', badge: { value: (c) => c.failingChecks, tone: 'warning' }, wholeTarget: true },
			{ label: 'Changes', href: '/changes', icon: GitCompareIcon, loads: 'listSnapshotChanges', wholeTarget: true }
		]
	},
	{
		section: 'Operate',
		items: [
			{ label: 'Runs', href: '/runs', icon: ListIcon, loads: 'listRunRows', badge: { value: (c) => c.activeRuns, tone: 'live' }, wholeTarget: true },
			{ label: 'Jobs', href: '/jobs', icon: PlayIcon, loads: 'listJobs', wholeTarget: true },
			{ label: 'Triggers', href: '/triggers', icon: ClockIcon, loads: 'listTriggers', badge: { value: (c) => c.pausedTriggers, tone: 'muted' }, wholeTarget: true }
		]
	},
	{
		section: 'Platform',
		items: [
			{ label: 'Locations', href: '/locations', icon: MapPinIcon, loads: 'listLocations', badge: { value: (c) => c.activePlatformEvents, tone: 'error' } },
			{ label: 'Connections', href: '/connections', icon: PlugIcon, loads: 'listWarehouseConnections' },
			{ label: 'Providers', href: '/providers', icon: PuzzleIcon, loads: 'listProviders' },
			{ label: 'Maintenance', href: '/maintenance', icon: WrenchIcon, loads: 'maintenanceHistory', wholeTarget: true }
		]
	},
	{
		section: 'Admin',
		requires: ADMIN_CAPABILITIES,
		items: [
			{ label: 'People', href: '/admin/people', icon: UsersIcon, loads: 'listAccounts', requires: ['user.manage', 'role.assign'] },
			{ label: 'Roles', href: '/admin/roles', icon: ShieldIcon, loads: 'listRoles', requires: ['role.assign'] },
			{ label: 'Audit log', href: '/admin/audit', icon: ScrollTextIcon, loads: 'listAuditEntries', requires: ['audit.view'] }
		]
	}
];

/** Entries the person may use and the server can load; groups left empty disappear. */
export function visibleNav(client: Pick<ApiClient, 'supports'>, capabilities: readonly Capability[]): NavGroup[] {
	return NAV.filter((g) => !g.requires || hasAnyCapability(capabilities, g.requires))
		.map((g) => ({
			...g,
			items: g.items.filter((i) => client.supports(i.loads) && (!i.requires || hasAnyCapability(capabilities, i.requires)))
		}))
		.filter((g) => g.items.length > 0);
}

/** Pages that don't list the selected target, so a person with view on only part of it can still use them. */
export function pagesBeyondTarget(client: Pick<ApiClient, 'supports'>, capabilities: readonly Capability[]): NavItem[] {
	return visibleNav(client, capabilities)
		.flatMap((g) => g.items)
		.filter((i) => !i.wholeTarget);
}

/** Whether the server can load the page at `path`; pages outside the navigation are not gated here. */
export function pageAvailable(client: Pick<ApiClient, 'supports'>, path: string): boolean {
	const item = NAV.flatMap((g) => g.items)
		.filter((i) => (i.href === '/' ? path === '/' : path === i.href || path.startsWith(`${i.href}/`)))
		.sort((a, b) => b.href.length - a.href.length)[0];
	return !item || client.supports(item.loads);
}
