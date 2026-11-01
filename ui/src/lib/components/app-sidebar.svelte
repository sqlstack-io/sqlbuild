<script lang="ts">
	import { page } from '$app/state';
	import CheckCircleIcon from '@lucide/svelte/icons/circle-check';
	import ClockIcon from '@lucide/svelte/icons/clock-3';
	import DatabaseIcon from '@lucide/svelte/icons/database';
	import GitBranchIcon from '@lucide/svelte/icons/git-branch';
	import GitCompareIcon from '@lucide/svelte/icons/git-compare';
	import BotIcon from '@lucide/svelte/icons/bot';
	import UsersIcon from '@lucide/svelte/icons/users';
	import LayoutDashboardIcon from '@lucide/svelte/icons/layout-dashboard';
	import ListIcon from '@lucide/svelte/icons/list';
	import MapPinIcon from '@lucide/svelte/icons/map-pin';
	import PlayIcon from '@lucide/svelte/icons/play';
	import PlugIcon from '@lucide/svelte/icons/plug';
	import PuzzleIcon from '@lucide/svelte/icons/puzzle';
	import ScrollTextIcon from '@lucide/svelte/icons/scroll-text';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import ShieldIcon from '@lucide/svelte/icons/shield';
	import WrenchIcon from '@lucide/svelte/icons/wrench';
	import type { Component } from 'svelte';
	import { ADMIN_CAPABILITIES, hasAnyCapability, type Capability, type NavCounts } from '$lib/api';
	import { app } from '$lib/state/app.svelte';
	import ScopeSwitcher from './shell/scope-switcher.svelte';

	type Badge = { value: (counts: NavCounts) => number; tone: 'muted' | 'warning' | 'error' | 'live' };
	type NavItem = { label: string; href: string; icon: Component; badge?: Badge; requires?: readonly Capability[] };
	type NavGroup = { section: string | null; items: NavItem[]; requires?: readonly Capability[] };

	const groups: NavGroup[] = [
		{ section: null, items: [{ label: 'Overview', href: '/', icon: LayoutDashboardIcon }] },
		{
			section: 'Build',
			items: [
				{ label: 'Lineage', href: '/lineage', icon: GitBranchIcon },
				{ label: 'Catalog', href: '/catalog', icon: DatabaseIcon, badge: { value: (c) => c.catalogNodes, tone: 'muted' } },
				{ label: 'Checks', href: '/checks', icon: CheckCircleIcon, badge: { value: (c) => c.failingChecks, tone: 'warning' } },
				{ label: 'Changes', href: '/changes', icon: GitCompareIcon }
			]
		},
		{
			section: 'Operate',
			items: [
				{ label: 'Runs', href: '/runs', icon: ListIcon, badge: { value: (c) => c.activeRuns, tone: 'live' } },
				{ label: 'Jobs', href: '/jobs', icon: PlayIcon },
				{ label: 'Triggers', href: '/triggers', icon: ClockIcon, badge: { value: (c) => c.pausedTriggers, tone: 'muted' } }
			]
		},
		{
			section: 'Platform',
			items: [
				{ label: 'Locations', href: '/locations', icon: MapPinIcon, badge: { value: (c) => c.activePlatformEvents, tone: 'error' } },
				{ label: 'Connections', href: '/connections', icon: PlugIcon },
				{ label: 'Providers', href: '/providers', icon: PuzzleIcon },
				{ label: 'Maintenance', href: '/maintenance', icon: WrenchIcon }
			]
		},
		{
			section: 'Admin',
			requires: ADMIN_CAPABILITIES,
			items: [
				{ label: 'People', href: '/admin/people', icon: UsersIcon, requires: ['user.manage', 'role.assign'] },
				{ label: 'Service accounts', href: '/admin/service-accounts', icon: BotIcon, requires: ['user.manage', 'token.manage'] },
				{ label: 'Roles', href: '/admin/roles', icon: ShieldIcon, requires: ['role.assign'], badge: { value: (c) => c.pendingApprovals, tone: 'warning' } },
				{ label: 'Audit log', href: '/admin/audit', icon: ScrollTextIcon, requires: ['audit.view'] }
			]
		}
	];

	const visibleGroups = $derived(
		groups
			.filter((g) => !g.requires || hasAnyCapability(app.capabilities, g.requires))
			.map((g) => ({ ...g, items: g.items.filter((i) => !i.requires || hasAnyCapability(app.capabilities, i.requires)) }))
			.filter((g) => g.items.length > 0)
	);

	const badgeTone: Record<Badge['tone'], string> = {
		muted: '#707782',
		warning: 'var(--sb-warning)',
		error: 'var(--sb-error)',
		live: 'var(--sb-running)'
	};

	const initials = $derived(
		(app.user?.displayName ?? '?')
			.split(/\s+/)
			.map((part) => part[0])
			.join('')
			.slice(0, 2)
			.toUpperCase()
	);

	function isActive(href: string): boolean {
		if (href === '/') return page.url.pathname === '/';
		return page.url.pathname === href || page.url.pathname.startsWith(`${href}/`);
	}

	function itemClass(href: string): string {
		return isActive(href)
			? 'bg-[var(--sidebar-accent)] text-foreground font-medium'
			: 'text-[var(--sidebar-foreground)] hover:bg-[var(--sb-hover)] hover:text-foreground';
	}
</script>

<aside class="app-sidebar bg-sidebar flex h-screen w-56 shrink-0 flex-col border-r border-[var(--sidebar-border)] px-3 py-4">
	<div class="flex items-center gap-2 px-2 pb-4">
		<img src="/logo.png" alt="SQLBuild" class="logo-dark h-6 w-auto" />
		<img src="/logo-light.png" alt="SQLBuild" class="logo-light h-6 w-auto" />
		{#if app.bootstrap}
			<span class="text-muted-foreground ml-auto text-[11px]" title="Server version">v{app.bootstrap.serverVersion}</span>
		{/if}
	</div>

	<ScopeSwitcher />

	<nav class="sb-nav flex min-h-0 flex-1 flex-col gap-px overflow-y-auto">
		{#each visibleGroups as group (group.section ?? 'root')}
			{#if group.section}
				<div class="text-muted-foreground px-3 pb-1 pt-4 text-[11.5px] font-medium">{group.section}</div>
			{/if}
			{#each group.items as item (item.href)}
				{@const Glyph = item.icon}
				{@const count = item.badge && app.navCounts ? item.badge.value(app.navCounts) : 0}
				<a href={item.href} class="relative flex items-center gap-2.5 rounded-md px-3 py-[7px] text-[13px] {itemClass(item.href)}">
					<span class="grid w-4 place-items-center opacity-80"><Glyph size={15} /></span>
					{item.label}
					{#if item.badge && count > 0}
						{#if item.badge.tone === 'live'}
							<span class="ml-auto flex items-center gap-1.5 text-[11.5px]" style:color={badgeTone.live}>
								<span class="h-1.5 w-1.5 rounded-full" style:background={badgeTone.live}></span>{count}
							</span>
						{:else}
							<span class="ml-auto text-[11.5px]" style:color={badgeTone[item.badge.tone]}>{count}</span>
						{/if}
					{/if}
				</a>
			{/each}
		{/each}
	</nav>

	<div class="mt-2 flex flex-col gap-px border-t border-[var(--sidebar-border)] pt-2.5">
		<a href="/settings" class="flex items-center gap-2.5 rounded-md px-3 py-[7px] text-[13px] {itemClass('/settings')}">
			<span class="grid w-4 place-items-center opacity-80"><SettingsIcon size={15} /></span>
			Settings
		</a>
		{#if app.user}
			<div class="text-[var(--sidebar-foreground)] flex items-center gap-2 px-2.5 py-1.5 text-[12.5px]">
				<span class="bg-primary grid h-[22px] w-[22px] shrink-0 place-items-center rounded-[2px] text-[10px] font-semibold text-white">{initials}</span>
				<span class="truncate">{app.user.displayName} · <span class="text-muted-foreground">{app.user.role}</span></span>
			</div>
		{/if}
	</div>
</aside>

<style>
	.app-sidebar {
		color: var(--foreground);
		box-shadow: 8px 0 30px rgba(15, 17, 20, 0.05);
	}
	:global(.dark) .app-sidebar {
		color: #f1f1ee;
		box-shadow: none;
	}
	.sb-nav {
		scrollbar-width: thin;
	}
	/* default (light theme): show the dark-colored logo */
	.logo-dark {
		display: none;
	}
	.logo-light {
		display: block;
	}
	/* dark theme: show the light-colored logo */
	:global(.dark) .logo-dark {
		display: block;
	}
	:global(.dark) .logo-light {
		display: none;
	}
</style>
