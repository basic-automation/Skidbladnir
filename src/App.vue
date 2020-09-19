<!-- eslint-disable max-len -->
<template>
        <div id="app" class="flex h-screen w-screen flex-col bg-gray-900 items-center text-gray-900 overflow-x-hidden p-8" @dragenter.prevent="appDragEnter()">
                <header class="flex flex-row bg-waves bg-bottom bg-cover justify-center">
                        <img src="./assets/logo.svg" class=" w-40">
                </header>
                <section class="grid grid-rows-2 md:h-10 md:grid-cols-2 gap-10 items-start md:justify-center flex-row max-w-6xl w-full mt-40 mx-4">
                        <FileUpload label="Input" type="input" dropZoneLabel="Drop Files Here" />
                        <FileUpload label="Output" type="output" dropZoneLabel="Drop Folders Here" />
                </section>
                <section class="grid grid-cols-1 items-center justify-center w-full">
                        <AdvancedOptionsToggle />
                </section>
                <section class="w-full min-h-40 flex-shrink-0"></section>
        </div>
        <div class="w-full h-40 absolute bottom-0 opacity-100 border-t border-gray-500 ba-glass antialiased"></div>
        <div class="w-full h-40 absolute bottom-0 opacity-100 border-t border-gray-500 ba-glass antialiased"></div>
        <div class="w-full h-40 absolute bottom-0 opacity-100 border-t border-gray-500 ba-glass antialiased"></div>
        <button class="flex items-center justify-center h-20 w-20 rounded-full absolute bg-gray-500 ba-submit-position border border-gray-500 ba-glass-button ba-submit-inner-shaddow-xl text-gray-100 focus:outline-none transition-all focus:ba-glass-glow focus:glow-xl ease-in-out duration-1000 ba-backdrop-transition" @focus="submitFocused=true" @blur="submitFocused=false">
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

                methods: {
                        appDragEnter: () => {
                                if(store.getters.appCanDragEnter) store.dispatch('setAppIsDragEnter', true);
                        },
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
                backdrop-filter: brightness(calc(21.5% * 3)) saturate(calc(160%)) blur(calc(.5rem / 3));
                background: repeating-radial-gradient(circle closest-corner at 50%, transparent .5vmin, rgba(160, 174, 192, .1) 1vmin);
                /* background: repeating-radial-gradient(closest-side at 40px 40px, transparent 9%, rgba(160, 174, 192, .18) 15%); */
                background-size: 100vw 100%;
        }

        .ba-glass-button {
                backdrop-filter: brightness(calc(21.5% * 3)) saturate(calc(160%)) blur(calc(.5rem / 3));
                background: repeating-radial-gradient(circle closest-corner at 2.5rem, transparent .5vmin, rgba(160, 174, 192, .1) 1vmin);
                /* background: repeating-radial-gradient(closest-side at 40px 40px, transparent 9%, rgba(160, 174, 192, .18) 15%); */
                background-size: 5rem 5rem;
        }

        .ba-glass-overlay {
                background: radial-gradient(transparent 60%, rgba(26, 32, 44, .1) 50%);
                background-size: 80px 80px;
        }

        .focus\:ba-glass-glow:focus {
                backdrop-filter: brightness(calc(17% * 3)) saturate(calc(160%)) blur(calc(.5rem / 3));
                background: repeating-radial-gradient(closest-side at 40px 40px, transparent 9%, rgba(247, 250, 252, .175) 15%);
                background-size: 80px 80px;                
        }

        .focus\:glow-xl:focus {
                box-shadow: 0 0px 5px rgba(247, 250, 252, 0.7), 0 0px 10px -5px rgba(247, 250, 252, 1);
        }

        .ba-submit-position {
                position: absolute;
                right: 15%;
                bottom: calc(10rem - 2.5rem);
                z-index: 2;
        }

        .ba-submit-inner-shaddow-xl {
                box-shadow: inset 0 0px 15px rgba(26, 32, 44, 1), 0 00px 10px -5px rgba(26, 32, 44, 1);
        }

        .min-h-40 {
                min-height: 10rem;
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
