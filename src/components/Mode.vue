<!-- eslint-disable max-len -->
<template>
        <div class="flex flex-col mt-8 flex-1 justify-center items-center transition-all ease-in-out duration-500 w-full">
                <h1 class="flex items-center text-gray-500 uppercase text-lg antialiased">Mode</h1>
                <div class="flex flex-row items-center justify-center gap-6 mt-6 flex-wrap">
                        <Radio group="Mode" id="jpegLike" label="JPEG Like" />
                        <Radio group="Mode" id="lossy" label="Lossy" />
                        <Radio group="Mode" id="lossless" label="Lossless" />
                        <Radio group="Mode" id="nearLossless" label="Near Lossless" />
                        <Radio group="Mode" id="presets" label="Presets" />
                </div>
        </div>
</template>

<script lang="ts">
        import { defineComponent } from 'vue';
        import store from '../store';

        export default defineComponent({
                name: 'Mode',

                computed: {
                        advancedOptionsIsShown: () => { return store.getters.advancedOptionsIsShown },
                },
        })
</script>