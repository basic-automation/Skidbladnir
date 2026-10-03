// The npm packages whose code the webview window's bundle carries, with their licence
// texts, for the third-party notices.
//
//   SKIDBLADNIR_JS_NOTICES=../LICENSES/third-party/javascript.md npm run generate
//
// then scripts/third-party-notices.py, which copies that file into both editions' notices.
// The list is read from what the client build actually emits — every module that kept
// code after tree-shaking — not from package.json, which names build tools that ship
// nothing and leaves out the dependencies that do. With the variable unset the build is
// unchanged; CI's window job sets it and fails if the committed file is stale.
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import type { Plugin } from 'vite'

interface Package { name: string, version: string, license: string, texts: string[] }

// Covered by their own sections of the notices: the font and the icon collections.
const elsewhere = (name: string) => name === '@fontsource-variable/fira-code' || name.startsWith('@iconify-json/')

/** The package a bundled file belongs to: the nearest package.json with a name and a version. */
function owner(file: string, cache: Map<string, Package | null>): Package | null {
	let directory = dirname(file)
	while (directory.includes('node_modules')) {
		const cached = cache.get(directory)
		if (cached !== undefined) return cached
		const manifest = join(directory, 'package.json')
		if (existsSync(manifest)) {
			const json = JSON.parse(readFileSync(manifest, 'utf8'))
			if (json.name && json.version) {
				const license = typeof json.license === 'string' ? json.license : (json.license?.type ?? 'not stated')
				const texts = readdirSync(directory).filter(entry => /^(licen[cs]e|copying|notice)/i.test(entry)).sort().map(entry => readFileSync(join(directory, entry), 'utf8').replace(/\r\n/g, '\n').trim())
				const found = { name: json.name, version: json.version, license, texts }
				cache.set(directory, found)
				return found
			}
		}
		directory = dirname(directory)
	}
	return null
}

export function javascriptNotices(): Plugin {
	const target = process.env.SKIDBLADNIR_JS_NOTICES
	return {
		name: 'skidbladnir:javascript-notices',
		apply: 'build',
		generateBundle(_options, bundle) {
			// Only the window's bundle: Nuxt's server build ships nothing in a static app.
			if (!target || this.environment.name !== 'client') return
			const cache = new Map<string, Package | null>()
			const packages = new Map<string, Package>()
			for (const output of Object.values(bundle)) {
				if (output.type !== 'chunk') continue
				for (const [id, module] of Object.entries(output.modules)) {
					if (module.renderedLength === 0 || !id.includes('/node_modules/')) continue
					const found = owner(id.replace(/\?.*$/, '').replace(/^\0/, ''), cache)
					if (found && !elsewhere(found.name)) packages.set(`${found.name}@${found.version}`, found)
				}
			}
			// One section per licence text, naming every package that carries it.
			const groups = new Map<string, Package[]>()
			for (const found of [...packages.values()].sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version))) {
				const key = found.texts.length ? found.texts.join('\n\n') : `(no licence file in the package; its package.json names ${found.license})`
				groups.set(key, [...(groups.get(key) ?? []), found])
			}
			const out: string[] = []
			for (const [text, members] of [...groups.entries()].sort((a, b) => a[1][0]!.name.localeCompare(b[1][0]!.name))) {
				const licenses = [...new Set(members.map(member => member.license))].join(', ')
				out.push(`### ${licenses}`, '', `Used by ${members.map(member => `\`${member.name}\` ${member.version}`).join(', ')}.`, '', '```text', text, '```', '')
			}
			writeFileSync(resolve(target), out.join('\n'))
		},
	}
}
