<template>
        <button class="w-full h-10 cursor-pointer focus:outline-none" @focus="focused=true" @blur="focused=false" @click.prevent="advancedOptionsClick()">
                <div class="flex flex-row items-center">
                        <hr class=" border-t border-gray-500 flex-1" :class="{ 'glow-xl': focused }" />
                        <div class="text-xs text-gray-500 flex justify-center items-center whitespace-no-wrap flex-shrink-0 mx-5 uppercase antialiased">{{ label }}</div>
                        <hr class=" border-t border-gray-500 flex-1" :class="{ 'glow-xl': focused }" />
                </div>
                <div class="flex h-3 text-gray-500 justify-center items-center mt-2" :class="{'animate-bounce': focused}">
                        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" :class="{ 'transform-rotate-180': !advancedOptionsIsShown }" class="h-3 transition-all ease-in-out duration-300">
                                <path class="fill-current" d="M6 0l6 12H0z" data-name="Polygon 1"/>
                        </svg>
                </div>
        </button>
</template>

<script lang="ts">
        import { defineComponent } from 'vue';
        import store from '../store';

        export default defineComponent({
                name: 'AdvancedOptionsToggle',

                data() {
                        return {
                                label: 'Advanced Options',
                                focused: false,
                        }
                },

                computed: {
                        advancedOptionsIsShown: () => { return store.getters.advancedOptionsIsShown },
                },

                methods: {
                        advancedOptionsClick: function() { store.dispatch('setAdvancedOptionsIsShown', !store.getters.advancedOptionsIsShown) },
                },

                watch: {
                        advancedOptionsIsShown: function() {
                                if(this.advancedOptionsIsShown) {
                                        this.label = 'Basic Options';
                                } else {
                                        this.label = 'Advanced Options';
                                }
                        },
                },
        });
</script>

<style scoped>
        .glow-xl {
                box-shadow: 0 0px 5px rgba(247, 250, 252, 1), 0 0px 10px -5px rgba(247, 250, 252, 1);
        }

        .transform-rotate-180 {
                transform: rotate(180deg);
        }

        .animate-bounce {
                -webkit-animation: shake-vertical 1s ease-in-out both;
                animation: shake-vertical 1s ease-in-out both;
        }

        @-webkit-keyframes shake-vertical {
                0%, 100% {
                        -webkit-transform: translateY(0);
                        transform: translateY(0);
                }
                10%, 30%, 50%, 70% {
                        -webkit-transform: translateY(-8px);
                        transform: translateY(-8px);
                }
                20%, 40%, 60% {
                        -webkit-transform: translateY(8px);
                        transform: translateY(8px);
                }
                80% {
                        -webkit-transform: translateY(6.4px);
                        transform: translateY(6.4px);
                }
                90% {
                        -webkit-transform: translateY(-6.4px);
                        transform: translateY(-6.4px);
                }
        }

        @keyframes shake-vertical {
                0%, 100% {
                        -webkit-transform: translateY(0);
                        transform: translateY(0);
                }
                10%, 30%, 50%, 70% {
                        -webkit-transform: translateY(-8px);
                        transform: translateY(-8px);
                }
                20%, 40%, 60% {
                        -webkit-transform: translateY(8px);
                        transform: translateY(8px);
                }
                80% {
                        -webkit-transform: translateY(6.4px);
                        transform: translateY(6.4px);
                }
                90% {
                        -webkit-transform: translateY(-6.4px);
                        transform: translateY(-6.4px);
                }
        }



</style>