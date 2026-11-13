<script lang="ts">
	import { appPath, to } from '$lib/paths';
	import logoForDark from '$website/src/assets/logo-dark.png';
	import logoForLight from '$website/src/assets/logo-light.png';
	import { page } from '$app/state';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import { visibleNav, type NavBadge } from '$lib/nav';
	import { app } from '$lib/state/app.svelte';
	import ScopeSwitcher from './shell/scope-switcher.svelte';

	const visibleGroups = $derived(visibleNav(app.client, app.capabilities));

	const badgeTone: Record<NavBadge['tone'], string> = {
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
		const path = appPath(page.url.pathname);
		if (href === '/') return path === '/';
		return path === href || path.startsWith(`${href}/`);
	}

	function itemClass(href: string): string {
		return isActive(href)
			? 'bg-[var(--sidebar-accent)] text-foreground font-medium'
			: 'text-[var(--sidebar-foreground)] hover:bg-[var(--sb-hover)] hover:text-foreground';
	}
</script>

<aside class="app-sidebar bg-sidebar flex h-full w-56 shrink-0 flex-col border-r border-[var(--sidebar-border)] px-3 py-4">
	<div class="flex items-center gap-2 px-2 pb-4">
		<img src={logoForLight} alt="SQLBuild" class="h-6 w-auto dark:hidden" />
		<img src={logoForDark} alt="SQLBuild" class="hidden h-6 w-auto dark:block" />
		{#if app.bootstrap?.serverVersion}
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
				<a href={to(item.href)} class="relative flex items-center gap-2.5 rounded-md px-3 py-[7px] text-[13px] {itemClass(item.href)}">
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
		{#if app.client.supports('serverSettings')}
		<a href={to(`/settings`)} class="flex items-center gap-2.5 rounded-md px-3 py-[7px] text-[13px] {itemClass('/settings')}">
			<span class="grid w-4 place-items-center opacity-80"><SettingsIcon size={15} /></span>
			Settings
		</a>
		{/if}
		{#if app.user}
			<div class="text-[var(--sidebar-foreground)] flex items-center gap-2 px-2.5 py-1.5 text-[12.5px]">
				<span class="bg-primary grid h-[22px] w-[22px] shrink-0 place-items-center rounded-[2px] text-[10px] font-semibold text-white">{initials}</span>
				<span class="truncate">{app.user.displayName}{#if app.user.role} · <span class="text-muted-foreground">{app.user.role}</span>{/if}</span>
				{#if app.signOut && app.bootstrap?.authMode !== 'disabled'}
					<button class="text-muted-foreground hover:text-foreground ml-auto text-[12px]" onclick={() => app.signOut?.()}>Sign out</button>
				{/if}
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
</style>
