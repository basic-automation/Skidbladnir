<template>
        <div :class="['flex flex-col flex-1 justify-center items-center transition-all ease-in-out duration-500 w-full', { 'hidden': !visible }]">
                <h1 class="flex items-center text-gray-500 uppercase text-lg antialiased">Deblocking Filter</h1>
                <div class="grid grid-cols-1 sm:grid-cols-3 items-start justify-center gap-4 mt-4 max-w-4xl">
                        <Radio group="deblocking" id="autoFilter" label="Auto Filter" />
                        <Radio group="deblocking" id="strongFilter" label="Strong Filter" />
                        <Radio group="deblocking" id="simpleFilter" label="Simple Filter" />
                </div>
                <div class="grid grid-cols-1 sm:grid-cols-2 items-start justify-center gap-4 mt-4 max-w-4xl">
                        <Slider group="strength" category="deblocking" dependency="deblocking" />
                        <Slider group="sharpness" category="deblocking" dependency="deblocking" :rtl="true" />
                </div>
        </div>
</template>

<script lang="ts">
        import { defineComponent } from 'vue';
        import store from '../store';

        export default defineComponent({
                name: 'Deblocking',

                data() {
                        return {
                                category: 'category',
                                group: 'CATdeblocking',
                                visible: false,
                        }
                },

                computed: {
                        advancedOptions: () => store.getters.advancedOptions,
                        categories: () => store.getters.componentCategory as [{ category: string; group: string; visible: boolean }]
                },

                watch: {
                        advancedOptions: {
                                deep: true,
                                handler() {
                                        if(this.group && this.category) {
                                                for(let i= 0; i < this.categories.length; i++) {
                                                        if(this.categories[i].group === this.group) {
                                                                this.visible = this.categories[i].visible;
                                                        }
                                                }
                                        }
                                }
                        },
                }
        })
</script>