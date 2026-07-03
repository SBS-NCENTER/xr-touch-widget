import { mount } from 'svelte';
import '../shared/tokens.css';
import Palette from '../widget/Palette.svelte';

// Busy fake backdrop so glass CSS can be tuned in a plain browser tab
// (browser tabs have no window transparency).
document.body.style.background =
  'linear-gradient(135deg,#0f2027,#203a43,#2c5364), ' +
  'repeating-linear-gradient(45deg, rgba(255,255,255,.08) 0 12px, transparent 12px 24px)';
document.body.style.minHeight = '100vh';

const host = document.getElementById('app');
host.style.padding = '80px';
mount(Palette, { target: host });
