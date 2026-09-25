import tailwindcss from '@tailwindcss/vite'

// Tauri serves this as static files from a bundled directory, so there is no Node
// server at runtime: `ssr: false` plus the static Nitro preset produce a pure SPA in
// `.output/public`, which is what `frontendDist` in src-tauri/tauri.conf.json points at.
export default defineNuxtConfig({
	ssr: false,

	// Tauri opens a fixed dev URL, so the port must not drift onto 3001 when something
	// else holds 3000.
	devServer: { port: 1420, host: '127.0.0.1' },

	// A desktop app has no crawler and no deep links to prerender.
	nitro: { preset: 'static' },

	css: ['~/assets/css/main.css'],
	vite: { plugins: [tailwindcss()], clearScreen: false },

	// The window is the whole product; devtools ride along only when asked for.
	devtools: { enabled: false },

	app: {
		// Tauri serves the bundle from the root of a custom protocol, so Nuxt's
		// root-absolute asset URLs resolve correctly and no baseURL override is needed.
		// `buildAssetsDir` only renames Nuxt's default `_nuxt` directory: a leading
		// underscore is awkward to reason about in a bundled app and some packagers skip
		// such paths.
		buildAssetsDir: 'assets',
		head: { title: 'Skidbladnir', meta: [{ name: 'color-scheme', content: 'light dark' }] },
	},
})
