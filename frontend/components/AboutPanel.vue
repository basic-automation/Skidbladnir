<script setup lang="ts">
// What this build is and the licences it is distributed under: the version, the edition,
// and the texts every installer carries (LICENSE, the third-party notices and, in the GPL
// edition, COPYING), read by the Rust core by fixed name.
import { computed, ref } from 'vue'
import type { Edition } from '~/composables/useSettings'
import { invokeCommand } from '~/composables/useTauri'

const props = defineProps<{ appVersion: string, backendVersion: string, edition: Edition | null }>()
const emit = defineEmits<{ close: [] }>()

type LegalDocument = 'license' | 'notices' | 'copying'
const documents = computed(() => {
	const list: { value: LegalDocument, label: string }[] = [
		{ value: 'license', label: 'Skidbladnir licence' },
		{ value: 'notices', label: 'Third-party notices' },
	]
	if (props.edition?.x265) list.push({ value: 'copying', label: 'GNU GPL v3' })
	return list
})

const shown = ref<LegalDocument | null>(null)
const text = ref('')
const error = ref('')
const loading = ref(false)

async function show(document: LegalDocument) {
	if (shown.value === document) {
		shown.value = null
		return
	}
	shown.value = document
	text.value = ''
	error.value = ''
	loading.value = true
	try {
		text.value = await invokeCommand<string>('legal_document', { document })
	}
	catch (failure) {
		error.value = failure instanceof Error ? failure.message : String(failure)
	}
	finally {
		loading.value = false
	}
}
</script>

<template>
	<ControlPanel title="About Skidbladnir">
		<template #actions>
			<UButton color="neutral" variant="ghost" icon="i-material-symbols-close" aria-label="Close About" :ui="{ base: 'p-1', leadingIcon: 'size-4' }" @click="emit('close')" />
		</template>
		<div class="flex flex-col gap-2 px-2.5 text-xs">
			<p class="text-paleday-bright" data-selectable>
				Skidbladnir {{ appVersion }}{{ edition?.label ?? '' }}, distributed under {{ edition?.license ?? 'its licence' }}.
			</p>
			<p v-if="edition?.x265" class="text-paleday-dim">
				This edition encodes HEIC with x265, which is GPL-licensed, so the build as a whole is under the GNU GPL v3 or later. Its complete source is attached to every release.
			</p>
			<p v-else-if="edition" class="text-paleday-dim">
				This edition encodes HEIC with Kvazaar. A GPL edition, with x265, is published beside it.
			</p>
			<p v-if="backendVersion" class="text-paleday-dim" data-selectable>
				{{ backendVersion }}
			</p>
			<div class="flex flex-wrap gap-2 pt-1">
				<UButton v-for="document in documents" :key="document.value" size="sm" color="neutral" variant="soft" :class="shown === document.value ? 'ring-1 ring-paleday-accent-text' : ''" :aria-pressed="shown === document.value" @click="show(document.value)">
					{{ document.label }}
				</UButton>
			</div>
			<p v-if="loading" class="text-paleday-dim" role="status">
				Reading…
			</p>
			<p v-if="error" class="text-paleday-error" role="alert">
				{{ error }}
			</p>
			<pre
				v-if="shown && text"
				class="max-h-96 overflow-auto rounded-lg bg-paleday-field p-3 font-mono text-[11px] leading-relaxed whitespace-pre-wrap text-paleday-fg"
				tabindex="0"
				:aria-label="documents.find(entry => entry.value === shown)?.label"
				data-selectable
			>{{ text }}</pre>
		</div>
	</ControlPanel>
</template>
