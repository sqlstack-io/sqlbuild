import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	compilerOptions: {
		// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
		runes: ({ filename }) => (filename.split(/[/\\]/).includes('node_modules') ? undefined : true)
	},
	kit: {
		// Wire types come from the client generated from the server's OpenAPI document.
		alias: { $control: '../clients/typescript/src' },
		// SPA mode: static build with client-side routing. There is no Node
		// server in production; the SQLBuild server serves the built assets.
		adapter: adapter({
			fallback: 'index.html',
			pages: 'build',
			assets: 'build',
			precompress: false
		})
	}
};

export default config;
