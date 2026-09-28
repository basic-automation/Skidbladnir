<script setup lang="ts">
// The conversion screen, in the "Nanna" layout (Figma N2M6tr7JQLg89RLM1O0reH, node
// 163:738): a frameless window with its own rounded frame and window buttons, a rail that
// picks the output format, a sidebar holding the queue, the destination and the saved
// presets, and one scrolling column of settings with the Convert action at its head. The
// controls are Nuxt UI components in the Paleday skin from main.css.
//
// The show/hide behaviour is reproduced because it is real encoder behaviour: the advanced
// controls only reach the encoder in lossy mode, so offering them elsewhere would promise
// an effect that will not happen.
//
// Help text is libwebp's own wording from `cwebp -longhelp`. The Electron app was not a
// source for it: its `-info` spans are live value readouts and it has one tooltip in total.
import { computed, onMounted, onUnmounted, ref, toRaw, watch } from 'vue'
import type { AvifSettings, ConversionReport, EncodeJob, Mode, OutputFormat, Preset } from '~/composables/useSettings'
import { formatBytes, usesLossyOptions, usesManualFilter } from '~/composables/useSettings'
import { invokeCommand, isTauri } from '~/composables/useTauri'

const settings = ref<EncodeJob | null>(null)
const backendVersion = ref('')
const appVersion = ref('')
const startupError = ref('')

const inputPaths = ref<string[]>([])
const outputDirectory = ref('')
const busy = ref(false)
const reports = ref<ConversionReport[]>([])
const failures = ref<{ path: string, message: string }[]>([])
const validationError = ref('')
const dragging = ref(false)
const dropRejected = ref(0)
let unlistenDrop: (() => void) | null = null

/** What the Rust core says about a path the user offered. */
interface WebpInfo { width: number, height: number, hasAlpha: boolean, hasAnimation: boolean, compression: 'lossy' | 'lossless' | 'mixed' }
interface PathInspection { path: string, supported: boolean, format: string | null, webp: WebpInfo | null }

const inspected = ref<PathInspection[]>([])

/** An image found by scanning a folder, with its position under the scan root. */
interface FoundImage { path: string, relative: string }
// Non-empty only when the selection came from a folder scan, which is what decides whether
// output mirrors the source tree or lands flat.
const scanned = ref<FoundImage[]>([])
const mirrorStructure = ref(true)
// Whether choosing a folder also takes the folders inside it. Off by default: a folder
// often holds earlier output in a subfolder, and sweeping that up is rarely what was meant.
const includeSubfolders = ref(false)
// The folder the queue was scanned from, so the scan can be redone when the option changes.
const scannedRoot = ref('')
const animatedInputs = computed(() => inspected.value.filter(entry => entry.webp?.hasAnimation))

/** Preferences as the Rust side stores them, plus why it fell back if it did. */
interface Preferences { settings: EncodeJob, outputDirectory: string | null }
interface LoadedPreferences { preferences: Preferences, fellBack: string | null }

const preferencesNotice = ref('')
const currentFile = ref('')
const currentPercent = ref(0)
const doneCount = ref(0)
const cancelling = ref(false)
const presets = ref<{ name: string, settings: EncodeJob }[]>([])
const presetName = ref('')
const presetError = ref('')
const presetFormOpen = ref(false)
// The preset last applied, marked in the sidebar until the controls move away from it.
const activePreset = ref('')
let unlistenProgress: (() => void) | null = null
const FALLBACK_MESSAGES: Record<string, string> = {
	unreadable: 'Your saved settings could not be read, so the defaults were loaded.',
	unparseable: 'Your saved settings file was damaged, so the defaults were loaded.',
	invalidSettings: 'Your saved settings were out of range, so the defaults were loaded.',
}

const FORMATS: { value: OutputFormat, label: string, icon: string }[] = [
	{ value: 'webp', label: 'WebP', icon: 'i-iconoir-webp-format' },
	{ value: 'avif', label: 'AVIF', icon: 'i-vscode-icons-file-type-avif' },
]

const BIT_DEPTH_ITEMS = [
	{ label: '10-bit', value: 'ten' },
	{ label: '8-bit', value: 'eight' },
] satisfies { label: string, value: AvifSettings['bitDepth'] }[]

const COLOR_MODEL_ITEMS = [
	{ label: 'YCbCr', value: 'ycbcr' },
	{ label: 'RGB', value: 'rgb' },
] satisfies { label: string, value: AvifSettings['colorModel'] }[]

const ALPHA_MODE_ITEMS = [
	{ label: 'Clean', value: 'clean', description: 'Recolour fully transparent pixels to whatever encodes cheapest.' },
	{ label: 'Keep', value: 'dirty', description: 'Keep the colour under transparency exactly as it is.' },
	{ label: 'Premultiplied', value: 'premultiplied', description: 'Store colour premultiplied by alpha.' },
] satisfies { label: string, value: AvifSettings['alphaMode'], description: string }[]

const MODES: { value: Mode, label: string, description: string }[] = [
	{ value: 'lossy', label: 'Lossy', description: 'Ordinary WebP. The only mode with the advanced controls.' },
	{ value: 'lossless', label: 'Lossless', description: 'Exact pixels, larger files. Keeps colour under transparency.' },
	{ value: 'nearLossless', label: 'Near-lossless', description: 'Lossless with preprocessing; quality sets the level.' },
	{ value: 'jpegLike', label: 'JPEG-like', description: 'Roughly match the size a JPEG of this quality would be.' },
	{ value: 'preset', label: 'Preset', description: 'Use one of libwebp\'s tuned parameter sets.' },
]

const PRESET_ITEMS = [
	{ label: 'Select a preset', value: null },
	{ label: 'Default', value: 'default' },
	{ label: 'Photo', value: 'photo' },
	{ label: 'Picture', value: 'picture' },
	{ label: 'Drawing', value: 'drawing' },
	{ label: 'Icon', value: 'icon' },
	{ label: 'Text', value: 'text' },
] satisfies { label: string, value: Preset | null }[]

const FILTER_ITEMS = [
	{ label: 'Auto', value: 'auto' },
	{ label: 'Simple', value: 'simple' },
	{ label: 'Strong', value: 'strong' },
]

const TARGET_ITEMS = [
	{ label: 'Use quality', value: 'none' },
	{ label: 'Target file size', value: 'size' },
	{ label: 'Target PSNR', value: 'psnr' },
]

const isWebp = computed(() => settings.value?.format === 'webp')
const isAvif = computed(() => settings.value?.format === 'avif')
const extension = computed(() => settings.value?.format ?? 'webp')
const lossy = computed(() => (settings.value && isWebp.value ? usesLossyOptions(settings.value.webp.mode) : false))
const manualFilter = computed(() => (settings.value ? usesManualFilter(settings.value.webp.filter) : false))
const basicOnly = computed(() => settings.value?.webp.mode === 'nearLossless' || settings.value?.webp.mode === 'preset')

// The quality slider is relabelled per mode, because libwebp genuinely reinterprets it.
const qualityLabel = computed(() => {
	switch (settings.value?.webp.mode) {
		case 'lossless': return 'Compression effort'
		case 'nearLossless': return 'Near-lossless level'
		default: return 'Quality'
	}
})
const qualityHelp = computed(() => {
	switch (settings.value?.webp.mode) {
		case 'lossless': return '0 is fastest and largest, 100 slowest and smallest. Not fidelity here.'
		case 'nearLossless': return '0 is maximum loss, 100 is off.'
		default: return 'quality factor (0: small .. 100: big)'
	}
})

// Target size and PSNR are mutually exclusive in libwebp, so the UI offers one choice.
type TargetKind = 'none' | 'size' | 'psnr'
const targetKind = ref<TargetKind>('none')
const targetSize = ref(51200)
const targetPsnr = ref(42)

watch([targetKind, targetSize, targetPsnr], () => {
	if (!settings.value) return
	if (targetKind.value === 'size') settings.value.webp.target = { kind: 'size', value: targetSize.value }
	else if (targetKind.value === 'psnr') settings.value.webp.target = { kind: 'psnr', value: targetPsnr.value }
	else settings.value.webp.target = null
})

onMounted(async () => {
	try {
		backendVersion.value = await invokeCommand<string>('encoder_version')
		const { getVersion } = await import('@tauri-apps/api/app')
		appVersion.value = await getVersion()
		// Preferences carry the defaults when there is nothing stored, so this is the only
		// place startup settings come from.
		const loaded = await invokeCommand<LoadedPreferences>('load_preferences')
		settings.value = loaded.preferences.settings
		outputDirectory.value = loaded.preferences.outputDirectory ?? ''
		// 'noFile' is a first launch, which is not worth telling anyone about.
		presets.value = await invokeCommand<{ name: string, settings: EncodeJob }[]>('list_presets')
		if (loaded.fellBack && loaded.fellBack !== 'noFile') {
			preferencesNotice.value = FALLBACK_MESSAGES[loaded.fellBack] ?? 'Your saved settings could not be used, so the defaults were loaded.'
		}
	}
	catch (error) {
		startupError.value = error instanceof Error ? error.message : String(error)
	}

	if (!isTauri()) return

	const { listen } = await import('@tauri-apps/api/event')
	unlistenProgress = await listen<{ inputPath: string, percent: number }>('conversion-progress', (event) => {
		currentFile.value = event.payload.inputPath
		currentPercent.value = event.payload.percent
	})

	// Tauri's NATIVE drag-drop, not the HTML5 kind: with the native handler enabled the
	// webview's own dragover/drop events never fire, so listening for those would look
	// correct and silently do nothing.
	const { getCurrentWebview } = await import('@tauri-apps/api/webview')
	unlistenDrop = await getCurrentWebview().onDragDropEvent(async (event) => {
		if (event.payload.type === 'over') { dragging.value = true; return }
		if (event.payload.type === 'leave') { dragging.value = false; return }
		if (event.payload.type !== 'drop') return
		dragging.value = false

		// Which files are usable is decided by the Rust core, by reading each file's leading
		// bytes — never by matching extensions here, which would let the UI accept something
		// the loader then refuses.
		const dropped = await invokeCommand<PathInspection[]>('inspect_dropped_paths', { paths: event.payload.paths })
		const usable = dropped.filter(entry => entry.supported)
		dropRejected.value = dropped.length - usable.length
		if (usable.length > 0) {
			scanned.value = []
			scannedRoot.value = ""
			inputPaths.value = usable.map(entry => entry.path)
			inspected.value = usable
		}
	})
})

onUnmounted(() => {
	unlistenDrop?.()
	unlistenProgress?.()
})

async function chooseInputs() {
	const { open } = await import('@tauri-apps/plugin-dialog')
	const picked = await open({ multiple: true, filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'jpe', 'jif', 'jfif', 'jfi', 'tif', 'tiff', 'webp', 'avif'] }] })
	scanned.value = []
	scannedRoot.value = ''
	if (Array.isArray(picked)) inputPaths.value = picked
	else if (typeof picked === 'string') inputPaths.value = [picked]
	await describeInputs()
}

/// Ask the core what the chosen files are, so the UI can describe them and warn about the
/// ones it cannot handle properly.
async function describeInputs() {
	if (inputPaths.value.length === 0) { inspected.value = []; return }
	try {
		inspected.value = await invokeCommand<PathInspection[]>('inspect_dropped_paths', { paths: inputPaths.value })
	}
	catch {
		inspected.value = []
	}
}

async function chooseFolder() {
	const { open } = await import('@tauri-apps/plugin-dialog')
	const picked = await open({ directory: true })
	if (typeof picked !== 'string') return
	scannedRoot.value = picked
	await scanQueuedFolder()
}

async function scanQueuedFolder() {
	scanned.value = await invokeCommand<FoundImage[]>('scan_folder', { directory: scannedRoot.value, recursive: includeSubfolders.value })
	inputPaths.value = scanned.value.map(entry => entry.path)
	await describeInputs()
}

// Changing the option with a folder queued rescans it, so the queue always matches what
// the checkbox says rather than what it said when the folder was chosen.
watch(includeSubfolders, () => {
	if (scannedRoot.value && !busy.value) scanQueuedFolder()
})

async function chooseOutput() {
	const { open } = await import('@tauri-apps/plugin-dialog')
	const picked = await open({ directory: true })
	if (typeof picked === 'string') outputDirectory.value = picked
}

const canConvert = computed(() => {
	if (!settings.value || busy.value) return false
	if (inputPaths.value.length === 0 || !outputDirectory.value) return false
	// The preset requirement belongs to WebP's preset mode; it does not block an AVIF run.
	return !(isWebp.value && settings.value.webp.mode === 'preset' && !settings.value.webp.preset)
})

async function convert() {
	if (!settings.value) return
	busy.value = true
	cancelling.value = false
	reports.value = []
	failures.value = []
	validationError.value = ''
	doneCount.value = 0
	currentPercent.value = 0
	currentFile.value = ''
	try {
		// Validate once against the encoder's own rules rather than a copy of them here.
		await invokeCommand<EncodeJob>('validate_settings', { settings: settings.value })
	}
	catch (error) {
		validationError.value = error instanceof Error ? error.message : String(error)
		busy.value = false
		return
	}

	// Sequential, so one failure is attributed to its own file and the rest still run.
	for (const input of inputPaths.value) {
		try {
			// A folder scan can reproduce the source tree in the destination; a flat
			// selection has no tree to reproduce.
			const found = scanned.value.find(entry => entry.path === input)
			const report = found && mirrorStructure.value
				? await invokeCommand<ConversionReport>('convert_scanned', { settings: settings.value, input, relative: found.relative, outputRoot: outputDirectory.value })
				: await invokeCommand<ConversionReport>('convert_image', { settings: settings.value, input, outputDirectory: outputDirectory.value })
			reports.value.push(report)
		}
		catch (error) {
			const message = error instanceof Error ? error.message : String(error)
			// A cancellation is what the user asked for, not a failure to report as one.
			if (message.includes('cancelled')) break
			failures.value.push({ path: input, message })
		}
		doneCount.value += 1
	}
	busy.value = false
	cancelling.value = false
	currentFile.value = ''

	// Remember what was used, after the run rather than on every slider drag: these are
	// settings that demonstrably got as far as the encoder.
	try {
		await invokeCommand<void>('save_preferences', { preferences: { settings: settings.value, outputDirectory: outputDirectory.value || null } })
	}
	catch {
		// Preferences are a convenience. Failing to store them must not look like a failed
		// conversion, which is what the user actually asked for and which succeeded.
	}
}

async function savePreset() {
	if (!settings.value) return
	presetError.value = ''
	try {
		presets.value = await invokeCommand('save_preset', { name: presetName.value, settings: settings.value })
		activePreset.value = presetName.value.trim()
		presetName.value = ''
		presetFormOpen.value = false
	}
	catch (error) {
		presetError.value = error instanceof Error ? error.message : String(error)
	}
}

async function deletePreset(name: string) {
	presetError.value = ''
	try {
		presets.value = await invokeCommand('delete_preset', { name })
		if (activePreset.value === name) activePreset.value = ''
	}
	catch (error) {
		presetError.value = error instanceof Error ? error.message : String(error)
	}
}

function applyPreset(preset: { name: string, settings: EncodeJob }) {
	// Copied, not aliased: editing the controls afterwards must not silently rewrite the
	// stored preset.
	settings.value = structuredClone(toRaw(preset.settings))
	activePreset.value = preset.name
	// The target radio is UI state derived from settings.webp.target, so bring it back in step.
	targetKind.value = settings.value.webp.target?.kind ?? 'none'
	if (settings.value.webp.target?.kind === 'size') targetSize.value = settings.value.webp.target.value
	if (settings.value.webp.target?.kind === 'psnr') targetPsnr.value = settings.value.webp.target.value
}

async function cancel() {
	cancelling.value = true
	try {
		await invokeCommand<void>('cancel_conversion')
	}
	catch {
		cancelling.value = false
	}
}

// The results of a run stay until the next run replaces them, or until they are dismissed.
function dismissResults() {
	reports.value = []
	failures.value = []
}

const converted = computed(() => reports.value.length > 0 && failures.value.length === 0 && !busy.value)

// Preview: encode one of the selected files into memory with the current settings and
// show it beside the original. Nothing is written to disk. The core returns both sides as
// data: URLs and computes the saving itself.
interface Preview { original: string, encoded: string, sourceBytes: number, encodedBytes: number, width: number, height: number, savingPercent: number | null }
const preview = ref<Preview | null>(null)
const previewFormat = ref<OutputFormat>('webp')
const previewPath = ref('')
const previewing = ref(false)
const previewError = ref('')
// Set when the settings or the chosen file change after a preview, so an out-of-date
// comparison is labelled as such rather than passed off as current.
const previewStale = ref(false)
// Not every webview can decode AVIF: WebKitGTK is built without it on some Linux
// distributions (Ubuntu 24.04's is one, and the AppImage bundles that build). The file
// itself is fine; only the on-screen comparison is lost, and the window says so.
const encodedUndisplayable = ref(false)
const previewItems = computed(() => inputPaths.value.map(path => ({ label: basename(path), value: path })))

watch(inputPaths, (paths) => {
	preview.value = null
	previewError.value = ''
	previewPath.value = paths[0] ?? ''
})
watch([settings, previewPath], () => {
	if (preview.value) previewStale.value = true
}, { deep: true })

// A hand edit after applying a preset means the controls no longer show that preset.
watch(settings, (now, before) => {
	// Applying a preset replaces the object; only an edit in place can move away from it.
	if (now !== before || !activePreset.value) return
	const stored = presets.value.find(preset => preset.name === activePreset.value)?.settings
	if (!stored || JSON.stringify(stored) !== JSON.stringify(now)) activePreset.value = ''
}, { deep: true })

const sidebarCollapsed = ref(false)

// The folder the queue came from, or the one most of it shares, as the sidebar's subtitle.
const queueFolder = computed(() => {
	if (inputPaths.value.length === 0) return 'Nothing queued'
	const folders = inputPaths.value.map(path => path.replace(/[\\/][^\\/]*$/, ''))
	let common = folders[0]!
	for (const folder of folders) {
		while (common && folder !== common && !folder.startsWith(common + '/') && !folder.startsWith(common + '\\')) {
			common = common.replace(/[\\/][^\\/]*$/, '')
		}
	}
	return common || folders[0]!
})

const queueMenu = computed(() => [[
	{ label: 'Choose images…', icon: 'i-lucide-images', onSelect: () => chooseInputs() },
	{ label: 'Choose a folder…', icon: 'i-lucide-folder-search', onSelect: () => chooseFolder() },
], [
	{
		label: 'Include subfolders',
		type: 'checkbox' as const,
		checked: includeSubfolders.value,
		onUpdateChecked: (checked: boolean) => { includeSubfolders.value = checked },
		// Keep the menu open, so the box can be ticked and then a folder chosen.
		onSelect: (event: Event) => event.preventDefault(),
	},
]])

const fileCount = computed(() => `${inputPaths.value.length} file${inputPaths.value.length === 1 ? '' : 's'}`)

// The window is frameless, so the app draws its own buttons for these.
async function windowAction(action: 'minimize' | 'toggleMaximize' | 'close') {
	if (!isTauri()) return
	const { getCurrentWindow } = await import('@tauri-apps/api/window')
	await getCurrentWindow()[action]()
}

async function runPreview() {
	if (!settings.value || !previewPath.value) return
	previewing.value = true
	previewError.value = ''
	try {
		const format = settings.value.format
		encodedUndisplayable.value = false
		preview.value = await invokeCommand<Preview>('preview_encode', { settings: settings.value, input: previewPath.value })
		previewFormat.value = format
		previewStale.value = false
	}
	catch (error) {
		preview.value = null
		previewError.value = String(error)
	}
	finally {
		previewing.value = false
	}
}

function basename(path: string): string {
	return path.split(/[\\/]/).pop() ?? path
}
</script>

<template>
	<!-- The window's own frame: the Tauri window is frameless and transparent, so this
	     rounded surface is the whole visible window. -->
	<div class="flex h-full overflow-hidden rounded-[32px] bg-paleday-bg text-paleday-fg">
		<h1 class="sr-only">
			Skidbladnir
		</h1>

		<div
			v-if="dragging"
			class="pointer-events-none fixed inset-4 z-50 flex items-center justify-center rounded-[28px] bg-paleday-bg/90 text-sm font-semibold text-paleday-accent-text outline-2 outline-dashed outline-paleday-accent"
		>
			Drop images to convert
		</div>

		<!-- Main menu: the logo, the output format, and the app settings. -->
		<nav class="flex shrink-0 flex-col items-center gap-2 px-4 pt-8 pb-4" aria-label="Main menu" data-tauri-drag-region>
			<img src="/skidbladnir-logo.svg" alt="Skidbladnir" width="55" height="64" class="h-16 w-[55px]" data-tauri-drag-region>
			<div class="flex flex-1 flex-col items-start gap-4 px-2 pt-8 pb-4">
				<URadioGroup
					v-if="settings"
					v-model="settings.format"
					:items="FORMATS"
					aria-label="Output format"
					variant="card"
					indicator="hidden"
					:ui="{
						fieldset: 'flex-col gap-4',
						item: 'border-0 border-l border-transparent rounded-none px-2 py-1 cursor-pointer bg-transparent hover:not-has-disabled:not-has-focus-visible:not-has-data-[state=checked]:border-transparent has-data-[state=checked]:border-primary has-data-[state=checked]:bg-transparent',
						wrapper: 'gap-0',
						icon: 'size-6',
						label: 'sr-only',
					}"
				/>
				<div class="flex-1" />
				<UPopover :content="{ side: 'right', align: 'end' }">
					<UButton
						color="neutral"
						variant="ghost"
						icon="i-iconamoon-settings-light"
						aria-label="App settings"
						:ui="{ base: 'px-2 py-1', leadingIcon: 'size-6' }"
					/>
					<template #content>
						<div class="flex w-72 flex-col gap-2 p-2 text-xs">
							<ControlToggle v-model="includeSubfolders" label="Include subfolders" help="Choosing a folder also takes every folder inside it. Off takes only the images directly in it." />
							<ControlToggle v-model="mirrorStructure" label="Recreate folder structure" help="When subfolders are included, mirror them in the destination. Off writes every file side by side." :disabled="!includeSubfolders" />
							<p v-if="backendVersion" class="px-2.5 pb-1 text-paleday-dim" data-selectable>
								{{ backendVersion }}
							</p>
						</div>
					</template>
				</UPopover>
			</div>
		</nav>

		<!-- Sidebar: what is queued, where it goes, and the saved presets. -->
		<aside v-if="!sidebarCollapsed" class="flex w-[272px] shrink-0 flex-col gap-3 p-4" aria-label="Queue and presets">
			<div class="flex justify-end" data-tauri-drag-region>
				<UButton
					color="neutral"
					variant="link"
					trailing-icon="i-subway-down-2"
					class="px-1 py-1.5 text-xs font-semibold text-paleday-fg"
					:ui="{ trailingIcon: 'size-4 rotate-90' }"
					@click="sidebarCollapsed = true"
				>
					Collapse
				</UButton>
			</div>

			<UDropdownMenu :items="queueMenu" :disabled="!isTauri()" :content="{ align: 'start' }">
				<button type="button" class="flex w-full flex-col gap-1 rounded-lg px-2.5 py-[7px] text-left transition-colors hover:bg-paleday-field/70" :disabled="!isTauri()">
					<span class="flex w-full items-center gap-2">
						<UIcon name="i-el-inbox-box" class="size-4 shrink-0" />
						<span class="min-w-0 flex-1 text-xs text-paleday-bright">Queue</span>
						<UBadge v-if="inputPaths.length" :label="String(inputPaths.length)" :ui="{ base: 'rounded-full bg-primary px-1.5 py-px text-[11px] font-semibold text-paleday-fg' }" />
					</span>
					<span class="w-full truncate text-xs text-paleday-dim" data-selectable>{{ queueFolder }}</span>
				</button>
			</UDropdownMenu>

			<button type="button" class="flex w-full flex-col gap-1 rounded-lg px-2.5 py-[7px] text-left transition-colors hover:bg-paleday-field/70" :disabled="!isTauri()" @click="chooseOutput">
				<span class="flex w-full items-center gap-2">
					<UIcon name="i-ic-baseline-upcoming" class="size-4 shrink-0" />
					<span class="min-w-0 flex-1 text-xs text-paleday-bright">Destination</span>
				</span>
				<span class="w-full truncate text-xs text-paleday-dim" data-selectable>{{ outputDirectory || 'Not chosen' }}</span>
			</button>

			<div class="flex items-center justify-between">
				<h2 class="text-[11px] font-semibold tracking-[0.88px] text-paleday-bright">
					Presets
				</h2>
				<UPopover v-model:open="presetFormOpen" :content="{ side: 'right', align: 'start' }">
					<UButton color="neutral" variant="ghost" icon="i-material-symbols-add" aria-label="Save the current settings as a preset" :disabled="!settings" :ui="{ base: 'p-0', leadingIcon: 'size-4' }" />
					<template #content>
						<form class="flex w-64 gap-2 p-2" @submit.prevent="savePreset">
							<UInput v-model="presetName" placeholder="Name these settings…" aria-label="Name for the preset" size="sm" class="flex-1" autofocus />
							<UButton type="submit" size="sm" :disabled="!presetName.trim()">
								Save
							</UButton>
						</form>
					</template>
				</UPopover>
			</div>

			<ul class="flex flex-col gap-0.5">
				<li v-for="preset in presets" :key="preset.name" class="group relative">
					<button
						type="button"
						class="w-full truncate border-l px-2.5 py-[7px] pr-8 text-left text-xs transition-colors"
						:class="activePreset === preset.name
							? 'border-paleday-brown font-semibold text-paleday-accent-text'
							: 'rounded-lg border-transparent text-paleday-bright hover:bg-paleday-field/70'"
						:aria-current="activePreset === preset.name ? 'true' : undefined"
						@click="applyPreset(preset)"
					>
						{{ preset.name }}
					</button>
					<UButton
						color="neutral"
						variant="ghost"
						icon="i-lucide-x"
						:aria-label="`Delete preset ${preset.name}`"
						class="absolute top-1/2 right-1 -translate-y-1/2 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
						:ui="{ base: 'p-1', leadingIcon: 'size-3.5' }"
						@click="deletePreset(preset.name)"
					/>
				</li>
			</ul>
			<p v-if="!presets.length" class="px-2.5 text-xs text-paleday-dim">
				No saved presets yet. Set the controls how you like them and press +.
			</p>
			<p v-if="presetError" class="px-2.5 text-xs text-paleday-error" role="alert">
				{{ presetError }}
			</p>

			<div class="flex-1" />
			<p v-if="appVersion" class="text-[10px] text-paleday-bright" data-selectable>
				v{{ appVersion }}
			</p>
		</aside>

		<main class="flex min-w-0 flex-1 flex-col gap-4 pb-1.5" :class="sidebarCollapsed ? 'pl-2' : 'pl-6'">
			<!-- The title bar: a drag region with the window buttons at its end. -->
			<div class="flex shrink-0 items-center gap-4" data-tauri-drag-region>
				<UButton
					v-if="sidebarCollapsed"
					color="neutral"
					variant="link"
					trailing-icon="i-subway-down-2"
					class="px-1 py-1.5 text-xs font-semibold text-paleday-fg"
					:ui="{ trailingIcon: 'size-4 -rotate-90' }"
					@click="sidebarCollapsed = false"
				>
					Expand
				</UButton>
				<div class="h-5 flex-1" data-tauri-drag-region />
				<div class="flex items-center">
					<UButton color="neutral" variant="ghost" icon="i-material-symbols-minimize" aria-label="Minimise" :ui="{ base: 'rounded-none p-4', leadingIcon: 'size-4' }" @click="windowAction('minimize')" />
					<UButton color="neutral" variant="ghost" icon="i-mdi-maximize" aria-label="Maximise" :ui="{ base: 'rounded-none p-4', leadingIcon: 'size-4' }" @click="windowAction('toggleMaximize')" />
					<UButton color="neutral" variant="ghost" icon="i-material-symbols-close" aria-label="Close" :ui="{ base: 'rounded-none py-4 pr-8 pl-4', leadingIcon: 'size-4' }" @click="windowAction('close')" />
				</div>
			</div>

			<div class="min-h-0 flex-1 overflow-y-auto pr-4 pb-6">
				<div class="flex flex-col gap-4">
					<!-- Submit: what will happen, and the button that does it. -->
					<div class="flex items-center justify-end gap-3 pr-2.5">
						<p class="min-w-0 truncate text-xs text-paleday-dim" data-selectable>
							<template v-if="inputPaths.length && outputDirectory">
								{{ fileCount }} → {{ outputDirectory }}
							</template>
							<template v-else-if="inputPaths.length">
								Choose a destination to convert {{ fileCount }}.
							</template>
							<template v-else>
								Drop images anywhere on the window, or choose them from the queue.
							</template>
						</p>
						<UButton
							v-if="!busy"
							size="lg"
							icon="i-codicon-debug-start"
							:disabled="!canConvert"
							:ui="{ base: 'shrink-0 gap-2 rounded-[6px] bg-paleday-violet px-4 py-2.5 text-sm font-semibold text-paleday-on-violet hover:bg-paleday-violet/90 disabled:bg-paleday-violet/60 disabled:opacity-100', leadingIcon: 'size-4' }"
							@click="convert"
						>
							Convert {{ inputPaths.length ? fileCount : '' }}
						</UButton>
						<UButton v-else color="error" variant="soft" size="lg" :loading="cancelling" @click="cancel">
							{{ cancelling ? 'Stopping…' : 'Cancel' }}
						</UButton>
					</div>

					<UpdateBanner />

					<UAlert
						v-if="preferencesNotice"
						color="warning"
						variant="soft"
						:description="preferencesNotice"
						close
						@update:open="preferencesNotice = ''"
					/>
					<UAlert
						v-if="startupError"
						color="warning"
						variant="soft"
						title="Not connected to the encoder"
						:description="startupError"
					/>

					<div v-if="scannedRoot || animatedInputs.length || dropRejected || (inspected.length === 1 && inspected[0]?.webp)" class="flex flex-col gap-1 px-2.5 text-xs">
						<div v-if="scannedRoot" class="flex flex-wrap items-start gap-x-6">
							<p class="min-w-0 flex-1 py-[7px] text-paleday-dim">
								Found {{ scanned.length }} image{{ scanned.length === 1 ? '' : 's' }} in that folder{{ includeSubfolders ? ', including subfolders. Symlinks are skipped.' : '. Subfolders are left out.' }}
								<template v-if="includeSubfolders">
									{{ mirrorStructure ? 'The folder structure is recreated in the destination.' : 'Every file is written side by side in the destination.' }}
								</template>
							</p>
							<ControlToggle v-model="includeSubfolders" label="Include subfolders" class="shrink-0" />
						</div>
						<p v-if="inspected.length === 1 && inspected[0]?.webp" class="text-paleday-dim" data-selectable>
							Already a WebP: {{ inspected[0]!.webp!.width }}&times;{{ inspected[0]!.webp!.height }},
							{{ inspected[0]!.webp!.compression }}{{ inspected[0]!.webp!.hasAlpha ? ', with alpha' : '' }}.
						</p>
						<p v-if="animatedInputs.length > 0" class="text-paleday-warning">
							{{ animatedInputs.length === 1 ? 'One queued file is an animated WebP' : `${animatedInputs.length} queued files are animated WebPs` }}.
							Skidbladnir encodes still images, so {{ animatedInputs.length === 1 ? 'it' : 'they' }} will be
							skipped with an error rather than converted.
						</p>
						<p v-if="dropRejected > 0" class="text-paleday-warning">
							{{ dropRejected }} dropped {{ dropRejected === 1 ? 'file was' : 'files were' }} not a
							PNG, JPEG, TIFF, WebP or AVIF and {{ dropRejected === 1 ? 'was' : 'were' }} skipped.
						</p>
					</div>

					<ControlPanel v-if="busy" title="Converting">
						<div class="px-2.5" role="status" aria-live="polite">
							<div class="flex items-baseline justify-between gap-3 text-xs">
								<span class="truncate text-paleday-bright" data-selectable>
									{{ currentFile ? basename(currentFile) : 'Starting…' }}
								</span>
								<span class="font-semibold tabular-nums text-paleday-accent-text">
									{{ doneCount }} / {{ inputPaths.length }}
								</span>
							</div>
							<UProgress v-model="currentPercent" :max="100" size="sm" class="mt-2" :ui="{ base: 'bg-paleday-dim' }" />
							<p class="mt-2 text-xs text-paleday-dim">
								<template v-if="isAvif">
									The AVIF encoder reports no progress while it works, so the bar moves only
									when a file starts and when it finishes. Cancel cannot stop a file mid-encode,
									but that file is then discarded rather than written.
								</template>
								<template v-else>
									libwebp does not always report a final 100%, so the bar can stop short of
									the end before a file finishes.
								</template>
							</p>
						</div>
					</ControlPanel>

					<UAlert v-if="validationError" color="error" variant="soft" role="alert" :description="validationError" />

					<ControlPanel v-if="reports.length || failures.length" title="Results">
						<template #actions>
							<UButton
								color="neutral"
								variant="ghost"
								icon="i-material-symbols-close"
								aria-label="Dismiss the results"
								:ui="{ base: 'p-1', leadingIcon: 'size-4' }"
								@click="dismissResults"
							/>
						</template>
						<p class="sr-only" role="status" aria-live="polite">
							{{ reports.length }} converted, {{ failures.length }} failed.
						</p>
						<!-- A big batch would push every setting off-screen, so the list scrolls on its own. -->
						<div v-if="reports.length" class="max-h-80 overflow-y-auto">
							<table class="w-full text-left text-xs">
								<thead class="sticky top-0 bg-paleday-bg text-paleday-dim">
									<tr>
										<th class="px-2.5 py-1 font-semibold">File</th>
										<th class="py-1 text-right font-semibold">Before</th>
										<th class="py-1 text-right font-semibold">After</th>
										<th class="py-1 text-right font-semibold">Change</th>
										<th class="py-1 pr-2.5 text-right font-semibold">Size</th>
									</tr>
								</thead>
								<tbody class="tabular-nums">
									<tr v-for="report in reports" :key="report.outputPath">
										<td class="px-2.5 py-1 text-paleday-bright" data-selectable>{{ basename(report.outputPath) }}</td>
										<td class="py-1 text-right">{{ formatBytes(report.sourceBytes) }}</td>
										<td class="py-1 text-right">{{ formatBytes(report.outputBytes) }}</td>
										<td v-if="report.savingPercent === null" class="py-1 text-right">—</td>
										<td v-else class="py-1 text-right font-semibold" :class="report.savingPercent >= 0 ? 'text-paleday-accent-text' : 'text-paleday-warning'">
											{{ report.savingPercent >= 0 ? '−' : '+' }}{{ Math.abs(report.savingPercent).toFixed(1) }}%
										</td>
										<td class="py-1 pr-2.5 text-right">{{ report.width }}×{{ report.height }}</td>
									</tr>
								</tbody>
							</table>
						</div>
						<ul v-if="failures.length" class="flex flex-col gap-1 px-2.5 text-xs text-paleday-error">
							<li v-for="failure in failures" :key="failure.path" data-selectable>
								<strong class="text-paleday-bright">{{ basename(failure.path) }}</strong>: {{ failure.message }}
							</li>
						</ul>
					</ControlPanel>

					<template v-if="settings">
						<ControlPanel v-if="isWebp" title="Mode" class="pt-8">
							<ChoiceGroup v-model="settings.webp.mode" :items="MODES" layout="cards" aria-label="Encoding mode" />
							<div v-if="settings.webp.mode === 'preset'" class="flex w-80 flex-col gap-2 px-2.5 py-[7px]">
								<span class="text-xs font-semibold text-paleday-fg">libwebp preset</span>
								<USelect v-model="settings.webp.preset" :items="PRESET_ITEMS" variant="soft" size="sm" aria-label="libwebp preset" :ui="{ base: 'bg-paleday-field' }" />
								<p v-if="!settings.webp.preset" class="text-xs text-paleday-warning">
									Pick a preset before converting.
								</p>
							</div>
						</ControlPanel>

						<ControlPanel v-if="isWebp" title="Quality" class="pt-8">
							<div class="grid grid-cols-3 gap-4">
								<ControlSlider v-model="settings.webp.quality" :label="qualityLabel" :min="0" :max="100" :help="qualityHelp" />
								<ControlSlider v-model="settings.webp.alphaQuality" label="Alpha quality" :min="0" :max="100" help="transparency-compression quality (0..100)" :disabled="basicOnly" />
								<ControlSlider v-model="settings.webp.method" label="Compression method" :min="0" :max="6" help="0 = fast, 6 = slowest and smallest" :disabled="basicOnly" />
							</div>
						</ControlPanel>

						<ControlPanel v-if="isAvif" title="Quality" class="pt-8">
							<div class="grid grid-cols-3 gap-4">
								<ControlSlider v-model="settings.avif.quality" label="Quality" :min="1" :max="100" help="colour quality (1: small .. 100: big)" />
								<ControlSlider v-model="settings.avif.alphaQuality" label="Alpha quality" :min="1" :max="100" help="transparency quality (1..100)" />
								<ControlSlider v-model="settings.avif.speed" label="Speed" :min="1" :max="10" help="1 = slowest and smallest, 10 = fastest and largest" />
							</div>
						</ControlPanel>

						<ControlPanel v-if="isAvif" title="AVIF">
							<div class="grid grid-cols-4 gap-3">
								<div class="flex flex-col gap-2 px-2.5 py-[7px]">
									<span class="text-xs font-semibold text-paleday-fg">Bit depth</span>
									<ChoiceGroup v-model="settings.avif.bitDepth" :items="BIT_DEPTH_ITEMS" aria-label="AVIF bit depth" />
									<p class="text-xs text-paleday-dim">
										10-bit keeps more precision through the colour conversion, even for 8-bit images.
									</p>
								</div>
								<div class="flex flex-col gap-2 px-2.5 py-[7px]">
									<span class="text-xs font-semibold text-paleday-fg">Colour model</span>
									<ChoiceGroup v-model="settings.avif.colorModel" :items="COLOR_MODEL_ITEMS" aria-label="AVIF colour model" />
									<p class="text-xs text-paleday-dim">
										RGB skips the colour conversion at a large size cost.
									</p>
								</div>
								<div class="col-span-2 flex flex-col gap-2 px-2.5 py-[7px]">
									<span class="text-xs font-semibold text-paleday-fg">Colour under transparency</span>
									<ChoiceGroup v-model="settings.avif.alphaMode" :items="ALPHA_MODE_ITEMS" orientation="vertical" aria-label="AVIF colour under transparency" />
								</div>
							</div>
							<div class="grid grid-cols-4 gap-3">
								<ControlToggle v-model="settings.avif.multiThreading" label="Multi-threading" help="encode on every core" />
							</div>
						</ControlPanel>

						<ControlPanel title="Resize">
							<div class="grid grid-cols-3 gap-3">
								<ControlToggle v-model="settings.resize.noEnlarge" label="Do not enlarge" help="Images already smaller than the target keep their size instead of being upscaled." />
								<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs">
									<span class="font-semibold text-paleday-fg">Width</span>
									<UInput v-model.number="settings.resize.width" type="number" :min="0" variant="soft" aria-label="Resize width in pixels" :ui="{ base: 'bg-paleday-rule text-paleday-fg hover:bg-paleday-rule focus:bg-paleday-rule', trailing: 'pe-2.5 text-paleday-fg' }">
										<template #trailing>
											px
										</template>
									</UInput>
									<p class="text-paleday-dim">
										0 derives it from the height, keeping the aspect ratio.
									</p>
								</div>
								<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs">
									<span class="font-semibold text-paleday-fg">Height</span>
									<UInput v-model.number="settings.resize.height" type="number" :min="0" variant="soft" aria-label="Resize height in pixels" :ui="{ base: 'bg-paleday-rule text-paleday-fg hover:bg-paleday-rule focus:bg-paleday-rule', trailing: 'pe-2.5 text-paleday-fg' }">
										<template #trailing>
											px
										</template>
									</UInput>
									<p class="text-paleday-dim">
										0 derives it from the width, keeping the aspect ratio.
									</p>
								</div>
							</div>
						</ControlPanel>

						<!-- The expert controls, behind a disclosure as in the Electron app, open by default. -->
						<UAccordion
							v-if="lossy"
							:items="[{ label: 'Advanced', value: 'advanced', slot: 'advanced' }]"
							default-value="advanced"
							:ui="{
								item: 'border-0',
								trigger: 'py-0 text-sm font-semibold text-paleday-bright',
								trailingIcon: 'size-4',
								body: 'pt-4 pb-0',
							}"
						>
							<template #advanced-body>
								<div class="flex flex-col gap-4">
									<div class="grid grid-cols-4 gap-3">
										<ControlSlider v-model="settings.webp.sns" label="Spatial noise shaping" :min="0" :max="100" help="0 = off, 100 = maximum" />
										<ControlSlider v-model="settings.webp.segments" label="Segments" :min="1" :max="4" help="number of segments to use (1..4)" />
										<ControlSlider v-model="settings.webp.partitionLimit" label="Partition limit" :min="0" :max="100" help="degradation allowed to fit the 512k prediction-mode limit" />
										<ControlSlider v-model="settings.webp.passes" label="Analysis passes" :min="1" :max="10" help="analysis pass number (1..10)" />
									</div>

									<div class="grid grid-cols-4 gap-3">
										<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px]">
											<span class="text-xs font-semibold text-paleday-fg">Deblocking filter</span>
											<ChoiceGroup v-model="settings.webp.filter" :items="FILTER_ITEMS" aria-label="Deblocking filter" />
											<p class="text-xs text-paleday-dim">
												Auto lets the encoder pick the strength, so the two sliders do not apply.
											</p>
										</div>
										<ControlSlider v-model="settings.webp.filterStrength" label="Filter strength" :min="0" :max="100" help="0 = off .. 100 = strongest" :disabled="!manualFilter" />
										<ControlSlider v-model="settings.webp.filterSharpness" label="Filter sharpness" :min="0" :max="7" help="0 = most sharp .. 7 = least sharp" :disabled="!manualFilter" />
									</div>

									<div class="grid grid-cols-4 gap-3">
										<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px]">
											<span class="text-xs font-semibold text-paleday-fg">Compression target</span>
											<ChoiceGroup v-model="targetKind" :items="TARGET_ITEMS" orientation="vertical" aria-label="Compression target" />
											<p class="text-xs text-paleday-dim">
												A size or PSNR target overrides the quality slider.
											</p>
										</div>
										<div v-if="targetKind === 'size'" class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs">
											<span class="font-semibold text-paleday-fg">Target size</span>
											<UInput v-model.number="targetSize" type="number" :min="1" variant="soft" aria-label="Target size in bytes" :ui="{ base: 'bg-paleday-rule text-paleday-fg hover:bg-paleday-rule focus:bg-paleday-rule', trailing: 'pe-2.5 text-paleday-fg' }">
												<template #trailing>
													bytes
												</template>
											</UInput>
											<p class="text-paleday-dim">
												Encoding stops as close to this output size as libwebp can get.
											</p>
										</div>
										<ControlSlider v-if="targetKind === 'psnr'" v-model="targetPsnr" label="Target PSNR (dB)" :min="1" :max="10000" help="typically around 42" />
									</div>

									<div class="grid grid-cols-4 gap-3">
										<ControlToggle v-model="settings.webp.sharpYuv" label="Sharp YUV" help="sharper (and slower) RGB to YUV" />
										<ControlToggle v-model="settings.webp.lowMemory" label="Low memory" help="less memory, slower encoding" />
										<ControlToggle v-model="settings.webp.multiThreading" label="Multi-threading" help="use multi-threading if available" />
									</div>
								</div>
							</template>
						</UAccordion>

						<ControlPanel v-if="inputPaths.length > 0 && !busy" title="Preview">
							<div class="flex items-center gap-2 px-2.5">
								<USelect v-if="previewItems.length > 1" v-model="previewPath" :items="previewItems" variant="soft" size="sm" aria-label="File to preview" class="w-80 shrink-0" :ui="{ base: 'bg-paleday-field text-paleday-bright' }" />
								<UButton
									:loading="previewing"
									:disabled="!previewPath"
									:ui="{ base: 'shrink-0 rounded-[6px] bg-primary px-3 py-[7px] text-xs font-semibold text-paleday-fg hover:bg-primary/85' }"
									@click="runPreview"
								>
									{{ preview ? 'Preview again' : 'Preview' }}
								</UButton>
								<p class="min-w-0 flex-1 text-xs text-paleday-dim">
									Encodes {{ previewItems.length > 1 ? 'the chosen file' : 'the file' }} in memory with the current settings. Nothing is written to disk.
								</p>
							</div>
							<p v-if="previewError" class="px-2.5 text-xs text-paleday-error" role="alert" data-selectable>
								{{ previewError }}
							</p>
							<div v-if="preview" class="flex flex-col gap-2" aria-live="polite">
								<p v-if="previewStale" class="px-2.5 text-xs text-paleday-warning">
									The settings or the file have changed since this preview. Preview again to see them.
								</p>
								<div class="grid grid-cols-2 gap-3">
									<figure class="flex min-w-0 flex-col gap-1.5 px-2.5 py-[7px]">
										<img :src="preview.original" alt="The original image" class="checkerboard max-h-[480px] w-full rounded-lg object-contain">
										<figcaption class="text-xs text-paleday-dim">
											Original · {{ formatBytes(preview.sourceBytes) }}
										</figcaption>
									</figure>
									<figure class="flex min-w-0 flex-col gap-1.5 px-2.5 py-[7px]">
										<img v-if="!encodedUndisplayable" :src="preview.encoded" :alt="`The image encoded as ${previewFormat.toUpperCase()}`" class="checkerboard max-h-[480px] w-full rounded-lg object-contain" @error="encodedUndisplayable = true">
										<p v-else class="flex h-[480px] items-center rounded-lg bg-paleday-field p-4 text-xs text-paleday-warning" role="note">
											This system's web view cannot display {{ previewFormat.toUpperCase() }}, so the encoded
											image cannot be shown here. The file Skidbladnir writes is unaffected, and the size and
											saving below are exact.
										</p>
										<figcaption class="text-xs text-paleday-dim">
											{{ previewFormat.toUpperCase() }} · {{ formatBytes(preview.encodedBytes) }} · {{ preview.width }}×{{ preview.height }}
											<template v-if="preview.savingPercent !== null">
												· <span class="font-semibold" :class="preview.savingPercent >= 0 ? 'text-paleday-accent-text' : 'text-paleday-warning'">{{ preview.savingPercent >= 0 ? '−' : '+' }}{{ Math.abs(preview.savingPercent).toFixed(1) }}%</span>
											</template>
										</figcaption>
									</figure>
								</div>
							</div>
						</ControlPanel>
					</template>
				</div>
			</div>
		</main>
	</div>
</template>
