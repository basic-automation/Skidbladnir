<template>
        <button ref="button" :class="['flex bg-gray-400 flex-col w-full md:w-auto items-center justify-center cursor-pointer flex-shrink-0 focus:outline-none transition-size duration-300 ease-in-out', { 'rounded-top-20px': isHover, 'rounded-20px': !isHover }]" @click="clicked(); isFocused = false" @focus="isFocused = true" @blur="isFocused = false" @mouseover="setMouseOverTrue()" @mouseout="setIsHoverFalse(); isMouseOver = false;">

                <div :class="['flex h-10 flex-row items-center justify-center w-full cursor-pointer flex-shrink-0 pointer-events-none']">
                        <label :class="['flex ml-4 justify-start items-center h-full text-xs cursor-pointer antialiased flex-shrink-0 pointer-events-none']">{{ label }}</label>
                        <div :class="['flex-1']"></div>
                        <div :class="['ba-radio ml-4 mr-2 border-gray-800 cursor-pointer antialiased flex-shrink-0 pointer-events-none', { 'ba-radio-checked': isSelected, 'ba-radio-focused': isFocused }]"></div>
                </div>

                <div :class="[{ 'w-0px h-0px': !isHover }, {'h-1': isHover}, isHover ? infoWidthClass : '', 'transition-size duration-1000 ease-in-out pointer-events-none']">
                        <hr :class="['flex border-gray-800 mx-4 mt-4 h-1 pointer-events-none']" />
                </div>

                <div ref="info" :class="['transition-size duration-300 ease-in-out overflow-y-scroll scrollbar-hidden pointer-events-none', {'h-0px w-0px': !isHover },{'p-4': isHover}, isHover ? infoHeightClass : '', isHover ? infoWidthClass : '' ]">
                        <p :class="['text-xs text-left pointer-events-none', { 'm-2': isHover }]">{{ infoLabel }}</p>
                </div>

        </button>

        <input :class="['hidden']" type="radio" :id="id" :name="group" :checked="isSelected" :value="value"/>
</template>

<script lang="ts">
        /* eslint-disable no-unused-vars */
        /* eslint-disable prefer-const */
        //import _ from 'lodash';
        import { defineComponent } from 'vue';
        import store from '../store';

        export default defineComponent({
                name: 'Radio',

                props: {
                        group: {
                                type: String,
                                required: true,
                        },
                        label: {
                                type: String,
                                required: true,
                        },
                        id: {
                                type: String,
                                required: true,
                        },
                },

                data() {
                        return {
                                value: this.id,
                                isSelected: false,
                                isFocused: false,
                                isHover: true,
                                isMouseOver: false,
                                hoverTimerDefault: 1000,
                                buttonWidth: 'auto',
                                infoLabel: '',
                                info: null as HTMLDivElement | null,
                                infoHeight: 0,
                                infoWidth: 0,
                                infoHeightClass: this.id + '-height',
                                infoWidthClass: this.id + '-width',
                                infoHeightVarName: '--' + this.id + '-height',
                                infoWidthVarName: '--' + this.id + '-width',
                                infoCSSID: 'ba-' + this.id,
                                infoAnimationDuration: 300,
                                windowWidth: 0,
                                isAnimatingWidth: false,
                                isAnimatingHeight: false,
                                loggingInfoWidth: false,
                                loggingInfoHeight: false,
                        };
                },

                computed: {
                        advancedOptions: () => store.getters.advancedOptions,
                        advancedOptionsIsShown: () => store.getters.advancedOptionsIsShown,
                        radioGroup: () => store.getters.componentsRadio,
                        componentCategory: () => store.getters.componentCategory as [{ category: string; group: string; visible: boolean }],
                },

                created() {
                        // define current group
                        let mygroup: { group: string; id: string[]; value: boolean[] } = { group: this.group, id: [this.id], value: [this.isSelected] };

                        // register component with store
                        store.commit('registerComponentRadioGroup', mygroup);

                        // set info label from store
                        const mode = this.advancedOptions[this.group.toLowerCase()];
                        for(const [key, value] of Object.entries(mode)) {
                                if(typeof value === 'object') {
                                        let newVal = value as { value: boolean; label: string };
                                        if(key === this.id) {
                                                this.infoLabel = newVal.label;
                                        }
                                }
                        }

                        // set custom css height
                        const heightVar = ':root { --' + this.id + '-height: auto; }';
                        this.addCss(this.infoCSSID,heightVar);
                        const heightClass = '.' + this.infoHeightClass + ' { height: var(' + this.infoHeightVarName + '); }';
                        this.addCss(this.infoCSSID, heightClass);

                        // set custom css width
                        const widthVar = ':root { --' + this.id + '-width: full; }';
                        this.addCss(this.infoCSSID, widthVar);
                        const widthClass = '.' + this.infoWidthClass + ' { width: var(' + this.infoWidthVarName + '); }';
                        this.addCss(this.infoCSSID, widthClass);

                },

                mounted() {
                        this.info = this.$refs.info as HTMLDivElement;
                        window.addEventListener('resize', () => { this.windowWidth = window.innerWidth });
                },

                methods: {
                        clicked: function() {
                                let mygroup: { group: string; id: string[]; value: boolean[] } = { group: this.group, id: [this.id], value: [true] };
                                store.commit('registerComponentRadioGroup', mygroup);
                                store.dispatch('syncModeWithRadioGroup', { group: this.group, id: this.id });
                                store.commit('deployAssociation', this.value);
                        },

                        logInfoHeight: function() {
                                if(!this.loggingInfoHeight) {
                                        this.loggingInfoHeight = true;
                                        const root = document.documentElement;
                                        return new Promise((resolve, reject) => {
                                                let inter = setInterval(() => {
                                                        root.style.setProperty(this.infoHeightVarName, "auto");
                                                        if(this.info !== null && !this.isAnimatingHeight && this.info.clientHeight > 50) {
                                                                root.style.setProperty(this.infoHeightVarName, this.info.clientHeight + "px");
                                                                this.infoHeight = this.info.clientHeight;
                                                                if(this.infoHeight > 50) {
                                                                        this.loggingInfoHeight = false;
                                                                        resolve(resolve);
                                                                        clearInterval(inter);
                                                                } else {
                                                                        reject(reject);
                                                                        clearInterval(inter);
                                                                }
                                                                
                                                        }
                                                }, this.infoAnimationDuration);
                                        }).catch((err) => { console.error('logInfoHeight Error: ', err) });
                                }
                        },

                        logInfoWidth: function() {
                                if(!this.loggingInfoWidth) {
                                        this.loggingInfoWidth = true;
                                        const root = document.documentElement;
                                        return new Promise((resolve, reject) => {
                                                let inter = setInterval(() => {
                                                        root.style.setProperty(this.infoWidthVarName, "100%");
                                                        if(this.info !== null && !this.isAnimatingWidth && this.info.clientWidth > 100 && this.info.clientWidth < 10000) {
                                                                root.style.setProperty(this.infoWidthVarName, this.info.clientWidth + "px");
                                                                this.infoWidth = this.info.clientWidth;
                                                                if(this.infoWidth > 100 && this.infoWidth < 10000) {
                                                                        this.loggingInfoWidth = false;
                                                                        resolve(resolve);
                                                                        clearInterval(inter);
                                                                } else {
                                                                        this.loggingInfoWidth = false;
                                                                        reject(reject);
                                                                        clearInterval(inter);
                                                                }
                                                        }
                                                }, this.infoAnimationDuration);
                                        }).catch((err) => { console.error('logInfoWidth Error: ', err) });
                                }
                        },

                        addCss: function(nodeID: string, cssCode: string) {
                                if(!document.getElementById(nodeID)){
                                        const styleElement = document.createElement('style') as HTMLStyleElement;
                                        styleElement.type = 'text/css';
                                        styleElement.id = nodeID;
                                        styleElement.appendChild(document.createTextNode(cssCode));
                                        document.getElementsByTagName('head')[0].appendChild(styleElement);
                                } else {
                                        const styleElement = document.getElementById(nodeID) as HTMLStyleElement;
                                        styleElement.type = 'text/css';
                                        styleElement.id = nodeID;
                                        let children = styleElement.childNodes;
                                        for(let i = 0; i < children.length; i++) {
                                                children[i].remove
                                        }
                                        styleElement.appendChild(document.createTextNode(cssCode));
                                        document.getElementsByTagName('head')[0].appendChild(styleElement);
                                }
                        },

                        setIsHoverFalse: function() {
                                return new Promise((resolve) => {
                                        let inter = setInterval(() => {
                                                if(!this.isAnimatingWidth && !this.isAnimatingHeight && !this.loggingInfoWidth && !this.loggingInfoHeight) {
                                                        this.isHover = false;
                                                        if(!this.isHover) {
                                                                resolve(resolve);
                                                                clearInterval(inter);
                                                        }
                                                }
                                        }, 100);
                                }).catch((err) => { console.error('setIsHoverFalse Error: ', err) });
                        },

                        setIsHoverTrue: function() {
                                return new Promise((resolve) => {
                                        let inter = setInterval(() => {
                                                this.isHover = true;
                                                if(this.isHover) {
                                                        resolve(resolve);
                                                        clearInterval(inter);
                                                }
                                        }, 10);
                                });
                        },

                        setMouseOverTrue: function() {
                                this.isMouseOver = true;
                        },

                        continueHover: function() {
                                return new Promise((resolve, reject) => {
                                        setTimeout(() => {
                                                if(this.isMouseOver == true) {
                                                        resolve(true);
                                                } else reject(false);
                                        }, this.hoverTimerDefault);
                                });
                        },
                
                },

                watch: {
                        windowWidth: function() {
                                if(store.getters.advancedOptionsIsShown) this.setIsHoverTrue().then(() => { this.logInfoHeight(); }).then(() => { this.logInfoWidth(); }).then(() => { this.setIsHoverFalse(); });
                        },

                        radioGroup: function() {
                                // find corresponding group and id and set checked value
                                for(let i = 0; i < this.radioGroup.length; i++) {
                                        if(this.radioGroup[i].group === this.group) {
                                                for(let j = 0; j < this.radioGroup[i].id.length; j++) {
                                                        if(this.radioGroup[i].id[j] === this.id) this.isSelected = this.radioGroup[i].value[j];
                                                }
                                        }
                                }
                        },

                        advancedOptions: {
                                deep: true,
                                handler() {
                                        const mode = this.advancedOptions[this.group.toLowerCase()];
                                        for(const [key, value] of Object.entries(mode)) {
                                                if(typeof value === 'object') {
                                                        let newVal = value as { value: boolean; label: string }
                                                        if(key === this.id) {
                                                                this.infoLabel = newVal.label
                                                        }
                                                }
                                        }
                                }
                        },

                        isHover: function() {
                                if(this.isHover) {
                                        this.isAnimatingWidth = true;
                                        this.isAnimatingHeight = true;
                                }
                        },

                        isAnimatingWidth: function() {
                                setTimeout(() => {
                                        this.isAnimatingWidth = false; 
                                }, this.infoAnimationDuration * 3);
                        },

                        isAnimatingHeight: function() {
                                setTimeout(() => {
                                        this.isAnimatingHeight = false;
                                }, this.infoAnimationDuration * 3);
                        },

                        advancedOptionsIsShown: function() {
                                if(this.advancedOptionsIsShown) {
                                        this.setIsHoverTrue().then(() => {
                                                this.logInfoHeight();
                                        }).then(() => { 
                                                this.logInfoWidth();
                                        }).then(() => {
                                                this.setIsHoverFalse();
                                        });
                                }
                        },

                        componentCategory: {
                                deep: true,
                                handler() {
                                        for(let i = 0; i < this.componentCategory.length; i++) {
                                                if(this.componentCategory[i].group === 'CATcompression' && this.componentCategory[i].visible === true) {
                                                        this.setIsHoverTrue().then(() => {
                                                                this.logInfoHeight();
                                                        }).then(() => { 
                                                                this.logInfoWidth();
                                                        }).then(() => {
                                                                this.setIsHoverFalse();
                                                        }).catch((err) => { console.error('componentCategory Error: ', err) });
                                                }
                                        }
                                }
                        },

                        isMouseOver: function() {
                                if(this.isMouseOver === true) Promise.all([this.continueHover()]).then((pass) => { if(pass) this.setIsHoverTrue(); console.log('isMouseOver') }).catch(() => { return });
                        }
                }
        });
</script>

<style scoped>

        :root {
                --info-height: auto;
                --info-width: 100%;
        }

        .info-height {
                height: var(--info-height);
        }

        .info-width {
                width: var(--info-width);
        }

        .ba-radio {
                display: block;
                width: 1.75em;
                height: 1.75em;
                border-radius: 50%;
                border: 0.1em solid;
        }

        .ba-radio-checked {
                background: radial-gradient(rgba(26, 32, 44, 1) 50%,rgba(26, 32, 44, 0) 55%);
                background-position: 50%, 50%;
        }

        .ba-radio-focused {
                background: radial-gradient(rgba(26, 32, 44, .5) 50%,rgba(26, 32, 44, 0) 55%);
                background-position: 50%, 50%;
        }

        .rounded-20px {
                border-radius: 20px;
        }

        .rounded-top-20px {
                border-top-right-radius: 20px;
                border-top-left-radius: 20px;
        }

        .h-0px {
                height: 0px;
        }

        .w-0px {
                width: 0px;
        }

        .w-330px {
                width: calc(330px - 4rem);
        }

        .scrollbar-hidden::-webkit-scrollbar {
                display: none;
        }
        
        .max-w-330px {
                max-width: calc(330px - 4rem);
        }
</style>