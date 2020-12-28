<template>
        <div :class="['flex flex-col flex-1 justify-center items-center transition-all ease-in-out duration-500 w-full', { 'hidden': !visible }]">
                <h1 class="flex items-center text-gray-400 uppercase text-lg antialiased">Quality</h1>
                <div class="grid grid-cols-1 sm:grid-cols-2 items-start justify-center gap-4 mt-4 max-w-4xl">
                        <Select group="preset" category="quality" dependency="mode" />
                        <Slider group="quality" category="quality"  dependency="mode" />
                        <Slider group="alphaQuality" category="quality" dependency="mode" />
                        <Slider group="compressionMethod" category="quality" dependency="mode" />
                </div>
        </div>
</template>

<script lang="ts">
        import { AdvancedOptions } from '@/interface/store/AdvancedOptions';
        import { Component } from '@/interface/store/Component';
        import { defineComponent } from 'vue';
        import store from '../store';

        export default defineComponent({
                name: 'Quality',

                data() {
                        return {
                                category: 'category',
                                group: 'CATquality',
                                visible: false,
                        }
                },

                computed: {
                        advancedOptions: () => store.getters.advancedOptions as AdvancedOptions,
                        categories: () => store.getters.componentCategory as Component[]
                },

                watch: {
                        advancedOptions: {
                                deep: true,
                                handler() { if(this.group && this.category) for(const component of this.categories) component.group === this.group ? this.visible = component.visible : false; }
                        },
                }
        })
</script>