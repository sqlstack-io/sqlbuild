// SPA mode: no SSR, no prerender. All data is fetched client-side, so the build
// is a static bundle served by the SQLBuild server.
export const ssr = false;
export const prerender = false;
export const trailingSlash = 'never';
