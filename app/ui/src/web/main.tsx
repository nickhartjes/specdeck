// The web viewer's entry (web.html), served by the sharing service (`web-viewer` design decision 5).

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import init, { FileFetch, FileUpload, WebViewer, parseJoinLink } from 'specdeck-web-wasm';
import { BridgeContext } from '../api/bridge';
import { applyAppearance } from '../appearance';
import '../styles.css';
import '../highlight.css';
import '../shared-files.css';
import '../links.css';
import '../asks.css';
import '../catch-up.css';
import { storedAppearance, webBridge } from './bridge';
import type { ControlUpdate, FetchUpdate, Request, StreamUpdate, ViewerCore } from './connection';
import { WebApp, takeHashCode } from './WebApp';
import { checkLink } from '../model/links';

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
    sendLink: (url, title, sessionOnly) => viewer.sendLink(url, title, sessionOnly) as Request,
    answerRead: (ask, answer) => viewer.answerRead(ask, answer) as Request,
    castVote: (ask, round, card) => viewer.castVote(ask, round, card) as Request,
    unansweredRequests: () => viewer.unansweredRequests() as Request[],
    shareFile: (name, bytes) => viewer.shareFile(name, bytes) as Request,
    addToBraindump: (name, bytes) => viewer.addToBraindump(name, bytes) as Request,
    addNote: (text, file) => viewer.addNote(text, file) as Request,
    removeFile: (file) => viewer.removeFile(file) as Request,
    fetchFile: (file, size, sha256) => {
      const fetch = new FileFetch(file, BigInt(size), sha256);
      return { request: () => fetch.request(), onReply: (bytes) => fetch.onReply(bytes) as FetchUpdate };
    },
    uploadFile: (upload, bytes) => {
      const sending = new FileUpload(upload, bytes);
      return {
        nextChunks: () => sending.nextChunks() as Uint8Array[],
        onReply: (bytes) => sending.onReply(bytes) as ReturnType<ReturnType<ViewerCore['uploadFile']>['onReply']>,
      };
    },
  };
}

/** A link in the form the page may open: the host's check, by the browser's own URL parser. */
function openableLink(url: string): string | undefined {
  const checked = checkLink(url);
  return 'url' in checked ? checked.url : undefined;
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
  const bridge = webBridge({ service, core, parseJoinLink, openableLink, storage: storage() });
  bridge.onAppearance(applyAppearance);
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <BridgeContext.Provider value={bridge}>
        <WebApp service={service} initialCode={initialCode} />
      </BridgeContext.Provider>
    </StrictMode>,
  );
});
