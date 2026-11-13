import { render } from 'preact';
import './style.css';
import { PanelApp } from './PanelApp';
import { start } from './panelState';

start();
const root = document.getElementById('app');
if (root) render(<PanelApp />, root);
