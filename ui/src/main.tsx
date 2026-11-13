import { render } from 'preact';
import './style.css';
import { Sidebar } from './Sidebar';
import { start } from './state';

start();
const root = document.getElementById('app');
if (root) render(<Sidebar />, root);
