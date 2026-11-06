import { browser } from '$app/environment';

type Theme = 'dark' | 'light';

function read(): Theme {
	if (!browser) return 'dark';
	return (localStorage.getItem('sb-theme') as Theme) ?? 'dark';
}

export const theme = $state<{ value: Theme }>({ value: read() });

function apply(t: Theme) {
	if (!browser) return;
	document.documentElement.classList.toggle('dark', t === 'dark');
	localStorage.setItem('sb-theme', t);
}

export function toggleTheme() {
	theme.value = theme.value === 'dark' ? 'light' : 'dark';
	apply(theme.value);
}
