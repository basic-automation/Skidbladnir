<script setup lang="ts">
// The Phase 1 shell. It proves the whole chain end to end — Nuxt renders, Tailwind
// styles, and the Tauri IPC boundary carries a real EncodeSettings round trip — without
// pretending to be the Phase 3 parity UI, which is where the actual controls land.
import { onMounted, ref } from 'vue'

const backend = ref<string | null>(null)
const failure = ref<string | null>(null)
const settings = ref<Record<string, unknown> | null>(null)

onMounted(async () => {
	try {
		// Imported lazily so the page still renders in a plain browser (`nuxt dev`
		// without Tauri), where this module has no host to talk to.
		const { invoke } = await import('@tauri-apps/api/core')
		backend.value = await invoke<string>('encoder_version')
		settings.value = await invoke<Record<string, unknown>>('default_settings')
	}
	catch (error) {
		failure.value = error instanceof Error ? error.message : String(error)
	}
})
</script>

<template>
	<main class="flex min-h-full flex-col gap-6 bg-white p-8 text-slate-900 dark:bg-slate-950 dark:text-slate-100">
		<header>
			<h1 class="text-2xl font-semibold tracking-tight">
				Skidbladnir
			</h1>
			<p class="mt-1 text-sm text-slate-600 dark:text-slate-400">
				Convert images to next-generation formats, with the whole encoder exposed.
			</p>
		</header>

		<section class="rounded-lg border border-slate-200 p-4 dark:border-slate-800">
			<h2 class="text-sm font-medium text-slate-500 uppercase dark:text-slate-400">
				Encode core
			</h2>
			<p v-if="backend" class="mt-2 font-mono text-sm" data-selectable>
				{{ backend }}
			</p>
			<p v-else-if="failure" class="mt-2 text-sm text-amber-600 dark:text-amber-400">
				Not connected to the Tauri backend ({{ failure }}). This page is running in a
				plain browser; launch it with <code>cargo tauri dev</code> to reach the encoder.
			</p>
			<p v-else class="mt-2 text-sm text-slate-500">
				Connecting&hellip;
			</p>
		</section>

		<section v-if="settings" class="rounded-lg border border-slate-200 p-4 dark:border-slate-800">
			<h2 class="text-sm font-medium text-slate-500 uppercase dark:text-slate-400">
				Default settings, from the Rust core
			</h2>
			<pre class="mt-2 overflow-x-auto text-xs leading-relaxed" data-selectable>{{ JSON.stringify(settings, null, 2) }}</pre>
		</section>

		<p class="mt-auto text-xs text-slate-500 dark:text-slate-500">
			Phase 1 shell. The encoder controls land in Phase 3 — see ROADMAP.md.
		</p>
	</main>
</template>
