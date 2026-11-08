// The real bridge: Tauri commands from app/src-tauri/src/lib.rs.

import { Channel, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { open, save } from '@tauri-apps/plugin-dialog';
import { exportFileName } from '../model/diagnostics';
import type { Bridge } from './bridge';
import type { MenuCommand } from '../menu/menu';
import type { AgentVersion, DiagnosticsStatus, HostEvent, JoinLink, Profile, RunningSession, ServiceCheck, UpdateState, ViewEvent } from './types';

function channel<T>(handler: (message: T) => void): Channel<T> {
  const ch = new Channel<T>();
  ch.onmessage = handler;
  return ch;
}

/**
 * Raw bytes from a channel. They arrive as an ArrayBuffer over the IPC protocol, but as a JSON
 * array of numbers when Tauri falls back to postMessage.
 */
export function asArrayBuffer(message: ArrayBuffer | ArrayBufferView | number[]): ArrayBuffer {
  if (message instanceof ArrayBuffer) return message;
  if (ArrayBuffer.isView(message)) return new Uint8Array(message.buffer, message.byteOffset, message.byteLength).slice().buffer;
  return new Uint8Array(message).buffer;
}

function bytesChannel(handler: (data: ArrayBuffer) => void): Channel<ArrayBuffer | ArrayBufferView | number[]> {
  return channel((message) => handler(asArrayBuffer(message)));
}

export const tauriBridge: Bridge = {
  getProfile: () => invoke<Profile>('get_profile'),
  agentVersions: () => invoke<AgentVersion[]>('agent_versions'),
  openInstallPage: (agent) => invoke('open_install_page', { agent }),
  setUsername: (username) => invoke<Profile>('set_username', { username }),
  setNotifications: (enabled) => invoke<Profile>('set_notifications', { enabled }),
  setCheckUpdates: (enabled) => invoke<Profile>('set_check_updates', { enabled }),
  setScreenReader: (enabled) => invoke<Profile>('set_screen_reader', { enabled }),
  setVerboseLogging: (enabled) => invoke<Profile>('set_verbose_logging', { enabled }),
  setBrowser: (enabled) => invoke<Profile>('set_browser', { enabled }),
  setScreenshots: (enabled) => invoke<Profile>('set_screenshots', { enabled }),
  updateStatus: () => invoke<UpdateState>('update_status'),
  onUpdate: (handler) => {
    const stop = listen<UpdateState>('update-changed', (event) => handler(event.payload));
    return () => void stop.then((unlisten) => unlisten());
  },
  restartToUpdate: () => invoke('restart_to_update'),
  setLayout: (layout) => invoke<Profile>('set_layout', { layout }),
  setSharingService: (service) => invoke<Profile>('set_sharing_service', { service }),
  checkSharingService: (service) => invoke<ServiceCheck>('check_sharing_service', { service }),
  removeRecentFolder: (path) => invoke<Profile>('remove_recent_folder', { path }),
  runningSessions: () => invoke<RunningSession[]>('running_sessions'),
  openRunningSession: (id) => invoke('open_running_session', { id }),
  endRunningSession: (id) => invoke('end_running_session', { id }),
  onRunningSessions: (handler) => {
    // Sent to every window at once (`running-sessions-on-start` design decision 2).
    const stop = listen<RunningSession[]>('running-sessions', (event) => handler(event.payload));
    return () => void stop.then((unlisten) => unlisten());
  },
  chooseFolder: async () => {
    const folder = await open({ directory: true, multiple: false, title: 'Choose the folder the agent works in' });
    return typeof folder === 'string' ? folder : null;
  },
  startHosting: (agent, folder, cols, rows) => invoke('start_hosting', { agent, folder, cols, rows }),
  hostAttach: (terminal, events) =>
    invoke('host_attach', { terminal: bytesChannel(terminal), events: channel<HostEvent>(events) }),
  hostInput: (data) => invoke('host_input', { data }),
  hostResize: (cols, rows) => invoke('host_resize', { cols, rows }),
  hostSetColors: (foreground, background) => invoke('host_set_colors', { foreground, background }),
  hostNewCode: () => invoke('host_new_code'),
  hostRemoveViewer: (viewerId) => invoke('host_remove_viewer', { viewerId }),
  hostSetViewerLimit: (limit) => invoke('host_set_viewer_limit', { limit }),
  hostAllowSuggestions: (on) => invoke('host_allow_suggestions', { on }),
  hostShareCode: (wholeFiles) => invoke('host_share_code', { wholeFiles }),
  hostAnswerSuggestion: (viewerId, id, text) => invoke('host_answer_suggestion', { viewerId, id, text }),
  hostClose: () => invoke('host_close'),
  joinSession: (code, server, terminal, events) =>
    invoke('join_session', { code, server, terminal: bytesChannel(terminal), events: channel<ViewEvent>(events) }),
  leaveSession: () => invoke('leave_session'),
  suggestPrompt: (text) => invoke('suggest_prompt', { text }),
  takeJoinLink: () => invoke<JoinLink | null>('take_join_link'),
  takeStartCommand: () => invoke<MenuCommand | null>('take_start_command'),
  setSessionTitle: (name) => invoke('set_session_title', { name }),
  readJoinLink: (link) => invoke<JoinLink>('read_join_link', { link }),
  copyText: (text) => navigator.clipboard.writeText(text),
  startScreenShown: () => invoke('start_screen_shown'),
  reportTerminal: (snapshot) => invoke('diagnostics_report_terminal', { snapshot }),
  diagnosticsStatus: () => invoke<DiagnosticsStatus>('diagnostics_status'),
  reconnectViewer: () => invoke('reconnect_viewer'),
  heartbeat: (beat) => invoke('diagnostics_heartbeat', { beat }),
  exportDiagnostics: async () => {
    const path = await save({
      defaultPath: exportFileName(new Date()),
      filters: [{ name: 'Diagnostics', extensions: ['jsonl'] }],
    });
    if (!path) return false;
    await invoke('diagnostics_export', { path });
    return true;
  },
  onMenu: (handler) => {
    // Only the commands sent to this window: the global `listen` also hears those sent to the
    // other windows (`multiple-windows` design decision 4).
    const stop = getCurrentWebviewWindow().listen<MenuCommand>('menu', (event) => handler(event.payload));
    return () => void stop.then((unlisten) => unlisten());
  },
};
