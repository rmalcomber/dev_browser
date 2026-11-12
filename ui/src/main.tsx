import { render } from 'preact';
import { activeWorkspace } from './state';

export function App() {
  return <p>The Dev Browser (alpha){activeWorkspace.value ? `, workspace ${activeWorkspace.value}` : ''}</p>;
}

const root = document.getElementById('app');
if (root) render(<App />, root);
