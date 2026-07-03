import { mount } from 'svelte';
import '../shared/tokens.css';
import Palette from './Palette.svelte';

mount(Palette, { target: document.getElementById('app') });
