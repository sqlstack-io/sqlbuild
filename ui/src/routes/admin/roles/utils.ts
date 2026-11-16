// Pure helpers for the Roles pages.

export type RolesTab = 'roles' | 'sso';

export function parseTab(value: string | null): RolesTab {
	return value === 'sso' ? value : 'roles';
}

export { plural } from '$lib/format/plural';
