<template>
        <div class="flex flex-col flex-1 justify-center items-center transition-all ease-in-out duration-500 w-full">
                <h1 class="flex items-center text-gray-500 uppercase text-lg antialiased">Quality</h1>
                <div class="grid grid-cols-1 sm:grid-cols-2 items-start justify-center gap-4 mt-4">
                        <Select group="preset" category="quality" dependency="mode" />
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