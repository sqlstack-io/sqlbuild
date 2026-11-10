import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	compilerOptions: {
		// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
		runes: ({ filename }) => (filename.split(/[/\\]/).includes('node_modules') ? undefined : true)
	},
	kit: {
		// Wire types come from the client generated from the server's OpenAPI document. The logos
		// and favicon are the website's approved files, so the two cannot drift apart.
		alias: { $control: '../clients/typescript/src', $website: '../website' },
		// `sqb server` serves the UI at the root; a build for another path, such as the static
		// demo at `/demo`, sets SQB_UI_BASE and links through `$lib/paths`.
		paths: { base: process.env.SQB_UI_BASE ?? '' },
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
