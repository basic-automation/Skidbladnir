<!-- eslint-disable max-len -->
<template>
        <div :class="['flex flex-col flex-1 justify-center items-center transition-all ease-in-out duration-500 w-full']">
                <h1 class="flex items-center text-gray-400 uppercase text-lg antialiased">Mode</h1>
                <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-5 items-start justify-center gap-4 mt-4 max-w-4xl">
                        <Radio category="Mode" group="Mode" id="jpegLike" label="JPEG Like" />
                        <Radio category="Mode" group="Mode" id="lossy" label="Lossy" />
                        <Radio category="Mode" group="Mode" id="lossless" label="Lossless" />
                        <Radio category="Mode" group="Mode" id="nearLossless" label="Near Lossless" />
                        <Radio category="Mode" group="Mode" id="preset" label="Presets" />
                </div>
        </div>
</template>

<script lang="ts">
        import { defineComponent } from 'vue';

        export default defineComponent({
                name: 'Mode',
        });
</script>