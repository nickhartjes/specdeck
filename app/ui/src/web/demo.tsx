// The web viewer's page on the demo bridge instead of a sharing service, for the how-to videos:
// the join page, then the demo session. Served by web-demo.html in development only.

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BridgeContext } from '../api/bridge';
import { demoBridge } from '../api/demo';
import { applyAppearance } from '../appearance';
import '../styles.css';
import '../highlight.css';
import '../shared-files.css';
import '../links.css';
import '../asks.css';
import '../notifications.css';
import '../catch-up.css';
import { DiagnosticLog } from './diagnostics';
import { WebApp, takeHashCode } from './WebApp';

const initialCode = takeHashCode();
document.documentElement.dataset.platform = 'web';
const bridge = demoBridge('viewer');
bridge.onAppearance(applyAppearance);
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <BridgeContext.Provider value={bridge}>
      <WebApp service="wss://specdeck.example.com/signal" initialCode={initialCode} diagnostics={new DiagnosticLog(null)} />
    </BridgeContext.Provider>
  </StrictMode>,
);
