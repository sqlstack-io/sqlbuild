// Pure helpers for the Roles pages.

export type RolesTab = 'roles' | 'sso';

export function parseTab(value: string | null): RolesTab {
	return value === 'sso' ? value : 'roles';
}

export function plural(n: number, one: string, many = `${one}s`): string {
	return `${n} ${n === 1 ? one : many}`;
}
