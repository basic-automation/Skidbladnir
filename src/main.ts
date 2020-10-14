import { createApp } from 'vue';
import App from './App.vue';
import './main.css';
import { store } from './store';
import FileUpload from './components/FileUpload.vue';
import AdvancedOptionsToggle from './components/AdvancedOptionsToggle.vue';
import Mode from './components/Mode.vue';
import Radio from './components/Radio.vue';
import Quality from './components/Quality.vue';
import Select from './components/Select.vue';
import Slider from './components/Slider.vue';
import Compression from './components/Compression.vue'

const app = createApp(App);
app.component('FileUpload', FileUpload);
app.component('AdvancedOptionsToggle', AdvancedOptionsToggle);
app.component('Mode', Mode);
app.component('Radio', Radio);
app.component('Quality', Quality);
app.component('Select', Select);
app.component('Slider', Slider);
app.component('Compression', Compression);
app.use(store);
app.mount('#app');
