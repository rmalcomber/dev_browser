import { render } from 'preact';
import { activeWorkspace } from './state';
import { connect, inApp, lastReply, ping } from './ipc';

export function App() {
  return (
    <div style="padding:12px;font:14px system-ui;color:#ccc">
      <div>
        The Dev Browser <small>(alpha){activeWorkspace.value ? `, workspace ${activeWorkspace.value}` : ''}</small>
      </div>
      <button id="ping" style="margin-top:12px" disabled={!inApp()} onClick={() => ping('hello')}>
        Ping
      </button>
      <div id="reply" style="margin-top:8px;color:#8fdde8">
        {inApp() ? (lastReply.value ?? 'no reply yet') : 'not running inside the app'}
      </div>
    </div>
  );
}

connect();
const root = document.getElementById('app');
if (root) render(<App />, root);
