#!/usr/bin/env node
// Checks that the TURN relay refuses to relay to private and special-purpose addresses, IPv4 and
// IPv6 alike, and still relays to public ones (sharing-service-limits). It takes credentials from
// the sharing service as a host would, allocates a relay and asks for a permission per address.
//
//   node scripts/turn-denied-peers.mjs ws://127.0.0.1:18081/signal
//
// Or straight against a relay, with its shared secret (for a dual-stack coturn in Docker):
//
//   node scripts/turn-denied-peers.mjs turn:[fd00:db8::2]:3478 <secret>
//
// Exits non-zero when a denied address is allowed or a public one refused. Run by `just local-test`.

import { createHash, createHmac, randomBytes } from 'node:crypto';
import dgram from 'node:dgram';

const DENIED = [
  '127.0.0.1',
  '10.0.0.1',
  '192.168.1.1',
  '169.254.169.254',
  '::1',
  '::ffff:10.0.0.1',
  '::a00:1',
  '64:ff9b::a00:1',
  '2002:a00:1::1',
  'fd00::1',
  'fe80::1',
];
const ALLOWED = ['1.1.1.1', '2606:4700:4700::1111'];

const MAGIC = 0x2112a442;
const ATTR = {
  username: 0x0006,
  integrity: 0x0008,
  error: 0x0009,
  peer: 0x0012,
  realm: 0x0014,
  nonce: 0x0015,
  family: 0x0017,
  transport: 0x0019,
};
const ALLOCATE = 0x0003;
const CREATE_PERMISSION = 0x0008;

async function credentials(signalling) {
  const socket = new WebSocket(signalling);
  const messages = [];
  let wake = () => {};
  socket.onmessage = ({ data }) => {
    messages.push(JSON.parse(String(data)));
    wake();
  };
  await new Promise((resolve, reject) => {
    socket.onopen = resolve;
    socket.onerror = () => reject(new Error(`cannot reach ${signalling}`));
  });
  // Only members get credentials: open a room first.
  socket.send(JSON.stringify({ type: 'host', room: randomBytes(16).toString('hex') }));
  socket.send(JSON.stringify({ type: 'turn-credentials' }));
  const deadline = Date.now() + 5000;
  for (;;) {
    const found = messages.find((m) => m.type === 'turn-credentials' || m.type === 'error');
    if (found) {
      socket.close();
      if (found.type === 'error') throw new Error(`no TURN credentials: ${found.code}`);
      return found;
    }
    if (Date.now() > deadline) throw new Error('no TURN credentials within 5 s');
    await new Promise((resolve) => {
      wake = resolve;
      setTimeout(resolve, 100);
    });
  }
}

function attribute(type, value) {
  const header = Buffer.alloc(4);
  header.writeUInt16BE(type, 0);
  header.writeUInt16BE(value.length, 2);
  const padding = Buffer.alloc((4 - (value.length % 4)) % 4);
  return Buffer.concat([header, value, padding]);
}

function xorPeer(address, id) {
  const v6 = address.includes(':');
  const value = Buffer.alloc(v6 ? 20 : 8);
  value[1] = v6 ? 2 : 1;
  value.writeUInt16BE(9 ^ (MAGIC >>> 16), 2);
  const bytes = v6 ? ipv6Bytes(address) : Buffer.from(address.split('.').map(Number));
  const mask = Buffer.concat([Buffer.from([0x21, 0x12, 0xa4, 0x42]), id]);
  bytes.forEach((b, i) => (value[4 + i] = b ^ mask[i]));
  return value;
}

function ipv6Bytes(address) {
  let text = address;
  // An embedded IPv4 address, as in ::ffff:10.0.0.1.
  const v4 = text.match(/(\d+\.\d+\.\d+\.\d+)$/);
  if (v4) {
    const [a, b, c, d] = v4[1].split('.').map(Number);
    text = text.replace(v4[1], `${((a << 8) | b).toString(16)}:${((c << 8) | d).toString(16)}`);
  }
  const [head, tail = ''] = text.split('::');
  const parts = (s) => (s ? s.split(':') : []);
  const missing = 8 - parts(head).length - parts(tail).length;
  const groups = [...parts(head), ...Array(text.includes('::') ? missing : 0).fill('0'), ...parts(tail)];
  const bytes = Buffer.alloc(16);
  groups.forEach((g, i) => bytes.writeUInt16BE(parseInt(g, 16), i * 2));
  return bytes;
}

/** A request with `attributes`, signed with `key` when given (long-term credentials). */
function request(type, attributes, key, id = randomBytes(12)) {
  let body = Buffer.concat(attributes);
  const header = Buffer.alloc(20);
  header.writeUInt16BE(type, 0);
  header.writeUInt32BE(MAGIC, 4);
  id.copy(header, 8);
  if (key) {
    header.writeUInt16BE(body.length + 24, 2);
    const mac = createHmac('sha1', key).update(Buffer.concat([header, body])).digest();
    body = Buffer.concat([body, attribute(ATTR.integrity, mac)]);
  }
  header.writeUInt16BE(body.length, 2);
  return { id, bytes: Buffer.concat([header, body]) };
}

function parse(message) {
  const attributes = new Map();
  for (let at = 20; at + 4 <= message.length; ) {
    const type = message.readUInt16BE(at);
    const length = message.readUInt16BE(at + 2);
    attributes.set(type, message.subarray(at + 4, at + 4 + length));
    at += 4 + length + ((4 - (length % 4)) % 4);
  }
  const error = attributes.get(ATTR.error);
  const code = error ? (error[2] & 7) * 100 + error[3] : 0;
  return { success: (message.readUInt16BE(0) & 0x0110) === 0x0100, code, attributes };
}

class Turn {
  constructor(host, port) {
    this.host = host;
    this.port = port;
    this.socket = dgram.createSocket(host.includes(':') ? 'udp6' : 'udp4');
  }

  exchange({ id, bytes }) {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.socket.off('message', onMessage);
        reject(new Error('no answer from the relay'));
      }, 3000);
      const onMessage = (message) => {
        if (!message.subarray(8, 20).equals(id)) return;
        clearTimeout(timer);
        this.socket.off('message', onMessage);
        resolve(parse(message));
      };
      this.socket.on('message', onMessage);
      this.socket.send(bytes, this.port, this.host);
    });
  }

  /** Allocates a relay of `family` (1 IPv4, 2 IPv6). Returns false when the relay has none. */
  async allocate(username, password, family) {
    const transport = attribute(ATTR.transport, Buffer.from([17, 0, 0, 0]));
    const familyAttr = attribute(ATTR.family, Buffer.from([family, 0, 0, 0]));
    const challenge = await this.exchange(request(ALLOCATE, [transport, familyAttr]));
    if (challenge.code !== 401) throw new Error(`expected a 401 challenge, got ${challenge.code}`);
    this.realm = challenge.attributes.get(ATTR.realm);
    this.nonce = challenge.attributes.get(ATTR.nonce);
    this.username = Buffer.from(username);
    this.key = createHash('md5').update(`${username}:${this.realm}:${password}`).digest();
    const answer = await this.signed(ALLOCATE, [transport, familyAttr]);
    if (answer.success) return true;
    if (answer.code === 440) return false;
    throw new Error(`allocation refused: ${answer.code}`);
  }

  signed(type, attributes, id) {
    const auth = [
      attribute(ATTR.username, this.username),
      attribute(ATTR.realm, this.realm),
      attribute(ATTR.nonce, this.nonce),
    ];
    return this.exchange(request(type, [...attributes, ...auth], this.key, id));
  }

  async permit(address) {
    // An IPv6 peer address is masked with the request's transaction id.
    const id = randomBytes(12);
    const peer = attribute(ATTR.peer, xorPeer(address, id));
    return this.signed(CREATE_PERMISSION, [peer], id);
  }
}

/** Credentials signed with the relay's shared secret, as the sharing service makes them. */
function signedCredentials(url, secret) {
  const username = `${Math.floor(Date.now() / 1000) + 600}:${randomBytes(8).toString('hex')}`;
  const credential = createHmac('sha1', secret).update(username).digest('base64');
  return { urls: [url], username, credential };
}

const [target = 'ws://127.0.0.1:18081/signal', secret] = process.argv.slice(2);
const { urls, username, credential } = target.startsWith('turn:')
  ? signedCredentials(target, secret)
  : await credentials(target);
const url = urls.find((u) => u.startsWith('turn:'));
if (!url) throw new Error(`no TURN URL in ${urls}`);
// turn:host:port, turn:[v6]:port, with an optional ?transport=udp.
const [, host, port = '3478'] = url.match(/^turn:\[?([^\]]+?)\]?(?::(\d+))?(?:\?.*)?$/);
console.log(`TURN relay ${host} port ${port}`);

let failures = 0;
for (const family of [1, 2]) {
  const turn = new Turn(host, Number(port));
  const name = family === 1 ? 'IPv4' : 'IPv6';
  if (!(await turn.allocate(username, credential, family))) {
    console.log(`${name} relay: none (the relay has no ${name} address)`);
    turn.socket.close();
    continue;
  }
  console.log(`${name} relay:`);
  for (const address of [...DENIED, ...ALLOWED]) {
    // Each relay reaches only its own family; coturn answers others with 443.
    if (address.includes(':') !== (family === 2)) continue;
    const answer = await turn.permit(address);
    const expected = ALLOWED.includes(address) ? 'allowed' : 'refused';
    const got = answer.success ? 'allowed' : `refused (${answer.code})`;
    const ok = answer.success === (expected === 'allowed') && (answer.success || answer.code === 403);
    if (!ok) failures++;
    console.log(`  ${ok ? 'ok  ' : 'FAIL'} ${address.padEnd(22)} ${got}, expected ${expected}`);
  }
  turn.socket.close();
}
process.exit(failures ? 1 : 0);
