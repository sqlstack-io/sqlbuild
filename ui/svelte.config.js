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
		// The served page carries this policy as a meta tag with the hash of SvelteKit's start
		// script; sqb server adds frame-ancestors, which a meta tag cannot set.
		csp: {
			mode: 'hash',
			directives: {
				'default-src': ['self'],
				'script-src': ['self'],
				'style-src': ['self', 'unsafe-inline'],
				'img-src': ['self', 'data:'],
				'connect-src': ['self'],
				'object-src': ['none'],
				'base-uri': ['none'],
				'form-action': ['self']
			}
		},
		adapter: adapter({
			fallback: 'index.html',
			pages: 'build',
			assets: 'build',
			precompress: false
		})
	}
};

export default config;
