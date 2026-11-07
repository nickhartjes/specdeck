// The web viewer's entry (web.html), served by the sharing service (`web-viewer` design decision 5).

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import init, { WebViewer, parseJoinLink } from 'specdeck-web-wasm';
import { BridgeContext } from '../api/bridge';
import { applyAppearance } from '../appearance';
import '../styles.css';
import { storedAppearance, webBridge } from './bridge';
import type { ControlUpdate, Request, StreamUpdate, ViewerCore } from './connection';
import { WebApp, takeHashCode } from './WebApp';

/** This page's own sharing service: `signal` next to the page, over WebSocket. */
function ownService(): string {
  const url = new URL('signal', location.href);
  url.protocol = url.protocol === 'http:' ? 'ws:' : 'wss:';
  url.hash = url.search = '';
  return url.href;
}

function core(code: string): ViewerCore {
  const viewer = new WebViewer(code);
  return {
    startAttempt: () => viewer.startAttempt(),
    signOffer: (connection, sdp) => viewer.signOffer(connection, sdp),
    verifyAnswer: (connection, description) => viewer.verifyAnswer(connection, description),
    hello: (username) => viewer.hello(username),
    leave: () => WebViewer.leave(),
    onControl: (bytes) => viewer.onControl(bytes) as ControlUpdate,
    onStream: (bytes) => viewer.onStream(bytes) as StreamUpdate,
    admittedOnce: () => viewer.admittedOnce(),
    suggest: (text) => viewer.suggest(text) as Request,
    unansweredRequests: () => viewer.unansweredRequests() as Request[],
  };
}

function storage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

const initialCode = takeHashCode();
document.documentElement.dataset.platform = 'web';
// Before anything renders, so the page does not switch after loading.
applyAppearance(storedAppearance(storage()));
void init().then(() => {
  const service = ownService();
  const bridge = webBridge({ service, core, parseJoinLink, storage: storage() });
  bridge.onAppearance(applyAppearance);
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <BridgeContext.Provider value={bridge}>
        <WebApp service={service} initialCode={initialCode} />
      </BridgeContext.Provider>
    </StrictMode>,
  );
});
