import { render } from 'preact';
import './style.css';
import { Launcher } from './Launcher';
import { start } from './launcherState';

start();
const root = document.getElementById('app');
if (root) render(<Launcher />, root);
