import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BridgeContext, type Bridge } from './api/bridge';
import { tauriBridge } from './api/tauri';
import { App } from './App';
import { applyAppearance } from './appearance';
import { SettingsScreen } from './screens/SettingsScreen';
import './styles.css';
import './highlight.css';
import './links.css';

async function bridge(): Promise<Bridge> {
  // In a plain browser during development, /?demo=start|host|viewer shows sample sessions.
  const demo = new URLSearchParams(location.search).get('demo');
  if (import.meta.env.DEV && demo && !('__TAURI_INTERNALS__' in window)) {
    const { demoBridge } = await import('./api/demo');
    return demoBridge(demo);
  }
  return tauriBridge;
}

void bridge().then((value) => {
  // The window opened with the appearance set (`appearance-setting` design decision 5); this
  // follows changes, and corrects a page reloaded since.
  value.onAppearance(applyAppearance);
  void value.getProfile().then((profile) => applyAppearance(profile.appearance));
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <BridgeContext.Provider value={value}>
        {new URLSearchParams(location.search).get('window') === 'settings' ? <SettingsScreen /> : <App />}
      </BridgeContext.Provider>
    </StrictMode>,
  );
});
