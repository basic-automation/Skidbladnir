<!-- eslint-disable max-len -->
<template>
        <div id="app" class="flex h-screen w-screen flex-col bg-gray-900 items-center text-gray-900 overflow-x-hidden p-8" @dragenter.prevent="appDragEnter()">
                <header class="flex flex-col justify-center items-center w-full">
                        <img src="./assets/logo.svg" class=" w-40">
                </header>
                <section class="flex flex-col md:flex-row item-center justify-center mt-12 gap-10 w-full max-w-4xl">
                        <FileUpload label="Input" type="input" dropZoneLabel="Drop Files Here" />
                        <FileUpload label="Output" type="output" dropZoneLabel="Drop Folders Here" />
                </section>
                <section class="flex flex-col items-center justify-center w-full mt-8">
                        <AdvancedOptionsToggle />
                </section>
                <section :class="{'grid grid-cols-1 items-center justify-center w-full mt-12': advancedOptionsIsShown, 'hidden': !advancedOptionsIsShown}">
                        <Mode />
                </section>
                <section :class="{'grid grid-cols-1 items-center justify-center w-full mt-12': advancedOptionsIsShown, 'hidden': !advancedOptionsIsShown}">
                        <Quality />
                </section>
                <section :class="{'grid grid-cols-1 items-center justify-center w-full mt-12': advancedOptionsIsShown, 'hidden': !advancedOptionsIsShown}">
                        <Compression />
                </section>
                <section :class="{'grid grid-cols-1 items-center justify-center w-full mt-12': advancedOptionsIsShown, 'hidden': !advancedOptionsIsShown}">
                        <Deblocking />
                </section>
                <section :class="{'grid grid-cols-1 items-center justify-center w-full mt-12': advancedOptionsIsShown, 'hidden': !advancedOptionsIsShown}">
                        <NoiseShaping />
                </section>
                <section class="w-full min-h-48 flex-shrink-0"></section>
        </div>
        <div class="w-full h-32 absolute bottom-0 opacity-100 border-t border-gray-500 ba-glass antialiased"></div>
        <div class="w-full h-32 absolute bottom-0 opacity-100 border-t border-gray-500 ba-glass antialiased"></div>
        <div class="w-full h-32 absolute bottom-0 opacity-100 border-t border-gray-500 ba-glass antialiased" :class="{'glow-xl': submitFocused}"></div>
        <div class="flex items-center justify-center h-20 w-20 rounded-full absolute bg-gray-500 ba-submit-position border border-gray-500 ba-glass-button ba-submit-inner-shaddow-xl text-gray-100 focus:outline-none transition-all focus:ba-glass-glow focus:glow-xl ease-in-out duration-1000 ba-backdrop-transition"></div>
        <div class="flex items-center justify-center h-20 w-20 rounded-full absolute bg-gray-500 ba-submit-position border border-gray-500 ba-glass-button ba-submit-inner-shaddow-xl text-gray-100 focus:outline-none transition-all focus:ba-glass-glow focus:glow-xl ease-in-out duration-1000 ba-backdrop-transition"></div>
        <button class="flex items-center justify-center h-20 w-20 rounded-full absolute bg-gray-500 ba-submit-position border border-gray-500 ba-glass-button ba-submit-inner-shaddow-xl text-gray-100 focus:outline-none focus:ba-glass-glow focus:glow-xl transition-all ease-in-out duration-300 ba-backdrop-transition" @focus="submitFocused=true" @blur="submitFocused=false" @click="imageSubmitted()">
                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 50 39" class="h-10"><path class="fill-current opacity-50" d="M42 0L19 23 8 13l-8 7 19 19L50 8z" data-name="Icon metro-checkmark"/></svg>
        </button>
</template>

<script lang="ts">
        import { defineComponent } from 'vue';
        import store from './store';

        export default defineComponent({
                name: 'app',

                props: {
                        msg: String,
                },

                data() {
                        return {
                                submitFocused: false,
                        }
                },

                computed: {
                        advancedOptionsIsShown: () => { return store.getters.advancedOptionsIsShown },
                },

                methods: {
                        appDragEnter: () => {
                                if(store.getters.appCanDragEnter) store.dispatch('setAppIsDragEnter', true);
                        },
                        imageSubmitted: () => {
                                store.dispatch('constructCMD');
                        }
                },
        });
</script>

<style scoped>
        @import url('https://fonts.googleapis.com/css2?family=Roboto+Slab:wght@700&display=swap');
        #app{
                font-family: 'Roboto Slab', serif;
                min-width: 5rem;
        }

        #app::-webkit-scrollbar {
                display: none;
        }

        .ba-glass {
                backdrop-filter: brightness(calc(24.25% * 3)) saturate(calc(130%)) blur(calc(.5rem / 3));
                background: repeating-radial-gradient(circle closest-corner at 50%, transparent .5vmin, rgba(160, 174, 192, .1) 1vmin);
                /* background: repeating-radial-gradient(closest-side at 40px 40px, transparent 9%, rgba(160, 174, 192, .18) 15%); */
                background-size: 100vw 100%;
        }

        .ba-glass-button {
                backdrop-filter: brightness(calc(24.5% * 3)) saturate(calc(130%)) blur(calc(.5rem / 3));
                background: repeating-radial-gradient(circle closest-corner at 2.5rem, transparent .65vmin, rgba(160, 174, 192, .1) 1vmin);
                /* background: repeating-radial-gradient(closest-side at 40px 40px, transparent 9%, rgba(160, 174, 192, .18) 15%); */
                background-size: 5rem 5rem;
        }

        .ba-glass-overlay {
                background: radial-gradient(transparent 60%, rgba(26, 32, 44, .1) 50%);
                background-size: 80px 80px;
        }

        .focus\:ba-glass-glow:focus {
                backdrop-filter: brightness(calc(17% * 3)) saturate(calc(160%)) blur(calc(.5rem / 3));
                background: repeating-radial-gradient(circle closest-corner at 2.5rem, transparent .636vmin, rgba(247, 250, 252, .1) 1vmin);
                /* background: repeating-radial-gradient(closest-side at 40px 40px, transparent 9%, rgba(247, 250, 252, .175) 15%); */
                background-size: 80px 80px;                
        }

        .focus\:glow-xl:focus {
                box-shadow: 0 0px 5px rgba(247, 250, 252, 0.3), 0 0px 10px -5px rgba(247, 250, 252, 1);
        }

        .glow-xl {
                box-shadow: 0 0px 5px rgba(247, 250, 252, 0.3), 0 0px 10px -5px rgba(247, 250, 252, 1);
        }

        .ba-submit-position {
                position: absolute;
                right: 10%;
                bottom: calc(8rem - 2.5rem);
                z-index: 2;
        }

        .ba-submit-inner-shaddow-xl {
                box-shadow: inset 0 0px 15px rgba(26, 32, 44, 1), 0 00px 10px -5px rgba(26, 32, 44, 1);
        }

        .min-h-48 {
                min-height: 12rem;
        }

        .ba-rotate-15-cw {
                -webkit-animation: rotate-15-cw .3s cubic-bezier(0.455, 0.030, 0.515, 0.955) both;
                animation: rotate-15-cw .3s cubic-bezier(0.455, 0.030, 0.515, 0.955) both;
        }

        @-webkit-keyframes rotate-15-cw {
                0% {
                        -webkit-transform: rotate(0);
                        transform: rotate(0);
                }
                100% {
                        -webkit-transform: rotate(15deg);
                        transform: rotate(15deg);
                }
        }
        
        @keyframes rotate-15-cw {
                0% {
                        -webkit-transform: rotate(0);
                        transform: rotate(0);
                }
                100% {
                        -webkit-transform: rotate(15deg);
                        transform: rotate(15deg);
                }
        }

</style>
