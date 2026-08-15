// 生成微信原生协议的 golden 向量（hex），供 Rust 单元测试逐字节比对。
//
// 用法（需 tsx）：
//   REF=/path/to/qq-farm-bot/core/src/services/wx-login/native-protocol.ts \
//     node ../qq-farm-bot/core/node_modules/tsx/dist/cli.mjs scripts/wx-golden-gen.mts
//
// 或设置 QQ_FARM_BOT_ROOT 指向 qq-farm-bot 仓库根目录。
//
// 原理：把参考实现 `core/src/services/wx-login/native-protocol.ts` 里的纯函数
// （不依赖真实 TCP 网络的部分）用「确定性 randomBytes + 固定 Date.now」stub 后
// 求值，输出 hex。这些函数仅依赖 node:crypto / node:net。
import { readFileSync, writeFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import crypto from 'node:crypto';
import net from 'node:net';

const REF =
  process.env.REF ??
  (process.env.QQ_FARM_BOT_ROOT
    ? join(
        process.env.QQ_FARM_BOT_ROOT,
        'core/src/services/wx-login/native-protocol.ts',
      )
    : undefined);

if (!REF) {
  console.error(
    'Set REF (path to native-protocol.ts) or QQ_FARM_BOT_ROOT (qq-farm-bot repo root).',
  );
  process.exit(1);
}

let src = readFileSync(REF, 'utf8');

// 1. 去掉 import，注入确定性 crypto / net（通过 globalThis）
src = src.replace("import crypto from 'node:crypto';", 'const crypto = (globalThis as any).__detCrypto;');
src = src.replace("import net from 'node:net';", 'const net = (globalThis as any).__detNet;');

// 2. 去掉 export（避免顶层 export 语法报错）
src = src.replace('export async function getNativeWxLoginCode', 'async function getNativeWxLoginCode');

// 3. 暴露纯函数
src += `
export {
  ch, pskClientHello, jsPlain, lz4Literal, lz4, wpkg, short, pbl, pbv, expand, nonce, manualRequest,
};
`;

// 写临时 .ts 文件，交给 tsx 转译
const dir = mkdtempSync(join(tmpdir(), 'wx-golden-'));
const tmpFile = join(dir, 'native-protocol.ts');
writeFileSync(tmpFile, src);

// 确定性 randomBytes：连续计数器填充
let counter = 0;
function detRandom(n: number): Buffer {
  const buf = Buffer.alloc(n);
  for (let i = 0; i < n; i++) buf[i] = (counter + i) & 0xff;
  counter += n;
  return buf;
}

// 固定时间戳
const FIXED_MS = 1700000000000; // 2023-11-14T22:13:20Z
const realNow = Date.now.bind(Date);
Date.now = () => FIXED_MS;

const fakeCrypto = Object.assign(Object.create(crypto), { randomBytes: detRandom });
(globalThis as any).__detCrypto = fakeCrypto;
(globalThis as any).__detNet = net;

const np = await import(tmpFile);

const hex = (b: Uint8Array) => Buffer.from(b).toString('hex');

// 固定输入
const pub1 = Buffer.from('04' + '11'.repeat(64), 'hex'); // 65 字节（含 04 前缀）
const pub2 = Buffer.from('04' + '22'.repeat(64), 'hex');
const timestamp = Math.floor(FIXED_MS / 1000);

const ticket = Buffer.from('aa'.repeat(100), 'hex');
const mac6 = Buffer.from([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);

// manualRequest 输入：base64 编码的 protobuf {1: ticket, 2: device, 3: host}
const appBytes = Buffer.from('ab'.repeat(32), 'hex');
const device = Buffer.from('cd'.repeat(16), 'hex');
const host = Buffer.from('wx'.repeat(16), 'hex');
const loginBufferRaw = Buffer.concat([
  np.pbl(1, ticket),
  np.pbl(2, device),
  np.pbl(3, host),
]);
const loginBuffer = loginBufferRaw.toString('base64');

const out = {
  clientHello: hex(np.ch(pub1, pub2)),
  pskClientHello: hex(np.pskClientHello(ticket, timestamp)),
  jsPlain: hex(np.jsPlain(12345678n, 'wxd44977328b36e647', Buffer.from('my_host_app_id'))),
  lz4LiteralShort: hex(np.lz4Literal(Buffer.from('hello'))),
  lz4LiteralLong: hex(np.lz4Literal(Buffer.from('A'.repeat(20)))),
  lz4Roundtrip: hex(np.lz4(np.lz4Literal(Buffer.from('the quick brown fox')))),
  wpkg: hex(np.wpkg({ 1: 1, 2: 12345 }, { 3: Buffer.from('xyz') })),
  short: hex(np.short(0x0d7d, 0, Buffer.from([1, 2, 3]))),
  pbl: hex(np.pbl(2, Buffer.from('abc'))),
  pbv: hex(np.pbv(2, 300)),
  expand: hex(np.expand(Buffer.from('secret'), 'label', Buffer.from('ctx'), 32)),
  nonce: hex(np.nonce(Buffer.from('0000000000000000000000ff', 'hex'), 7)),
  manualRequest: hex(np.manualRequest(loginBuffer, appBytes).req),
};

console.log(JSON.stringify(out, null, 2));
