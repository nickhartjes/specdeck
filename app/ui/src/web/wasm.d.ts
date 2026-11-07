// The WebAssembly package from crates/web (`just web-wasm`), as vite.web.config.ts aliases it.
// Declared here so the UI type-checks without building it first.
declare module 'specdeck-web-wasm' {
  export default function init(): Promise<unknown>;
  export class WebViewer {
    constructor(code: string);
    static protocolVersion(): number;
    static leave(): Uint8Array;
    admittedOnce(): boolean;
    startAttempt(): string;
    signOffer(connection: string, sdp: string): unknown;
    verifyAnswer(connection: string, description: unknown): string | undefined;
    hello(username: string): Uint8Array;
    onControl(bytes: Uint8Array): unknown;
    onStream(bytes: Uint8Array): unknown;
    suggest(text: string): unknown;
    unansweredRequests(): unknown;
  }
  export function parseJoinLink(link: string): { code: string; service?: string } | undefined;
}
