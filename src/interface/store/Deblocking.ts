import { RadioPreset } from './RadioPreset';
import { SliderPreset } from './SliderPreset';

export interface Deblocking {
        [name: string]: RadioPreset | SliderPreset;
}