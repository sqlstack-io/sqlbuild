// Apply the saved theme before first paint; the default is dark.
try {
	if ((localStorage.getItem('sb-theme') || 'dark') === 'dark') document.documentElement.classList.add('dark');
} catch (e) {
	document.documentElement.classList.add('dark');
}
