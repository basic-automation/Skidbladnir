<script setup lang="ts">
// Offers a newer release when one exists. The check runs once, on mount, in Rust; this
// component only shows the answer. It shows nothing at all unless there is an update:
// being offline, or a release that has no build for this platform yet, is not news.
//
// Nothing is downloaded until the user clicks Install. The install then restarts the app
// into the new version, so the button says so.
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { invokeCommand, isTauri } from '~/composables/useTauri'

interface UpdateInfo { version: string, currentVersion: string }

const update = ref<UpdateInfo | null>(null)
const dismissed = ref(false)
const installing = ref(false)
const downloaded = ref(0)
const total = ref<number | null>(null)
const installError = ref('')
let unlisten: (() => void) | null = null

const percent = computed(() => (total.value ? Math.min(100, Math.round((downloaded.value / total.value) * 100)) : null))

const description = computed(() => {
	if (!update.value) return ''
	if (installError.value) return `The update could not be installed: ${installError.value}`
	if (installing.value) return percent.value === null ? 'Downloading…' : `Downloading… ${percent.value}%`
	return `You have ${update.value.currentVersion}. Installing restarts Skidbladnir.`
})

async function install() {
	installing.value = true
	installError.value = ''
	downloaded.value = 0
	total.value = null
	try {
		// Resolves only on failure: on success the app restarts (or, on Windows, the
		// installer takes over) before this returns.
		await invokeCommand('install_update')
	}
	catch (error) {
		installError.value = error instanceof Error ? error.message : String(error)
	}
	finally {
		installing.value = false
	}
}

onMounted(async () => {
	if (!isTauri()) return
	const { listen } = await import('@tauri-apps/api/event')
	unlisten = await listen<{ downloaded: number, total: number | null }>('update-progress', (event) => {
		downloaded.value = event.payload.downloaded
		total.value = event.payload.total
	})
	try {
		update.value = await invokeCommand<UpdateInfo | null>('check_for_update')
	}
	catch {
		// Deliberately silent; see the note at the top.
	}
})

onUnmounted(() => unlisten?.())
</script>

<template>
	<UAlert
		v-if="update && !dismissed"
		color="primary"
		variant="subtle"
		icon="i-lucide-download"
		:title="`Skidbladnir ${update.version} is available`"
		:description="description"
		:actions="[{ label: installError ? 'Try again' : 'Install and restart', color: 'primary', loading: installing, disabled: installing, onClick: install }]"
		:close="!installing"
		orientation="horizontal"
		class="mb-6"
		data-testid="update-banner"
		@update:open="dismissed = true"
	/>
</template>
