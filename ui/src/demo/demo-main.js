import { mount } from 'svelte';
import '../shared/tokens.css';
import Demo from './Demo.svelte';

mount(Demo, { target: document.getElementById('app') });
