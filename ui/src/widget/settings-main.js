import { mount } from 'svelte';
import '../shared/tokens.css';
import '../shared/platform.js';
import Settings from './Settings.svelte';

mount(Settings, { target: document.getElementById('app') });
