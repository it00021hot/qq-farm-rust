import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { fetchWxLocalAuthorize, fetchWxLocalCheckLogin } from '../src/service/api/farm.ts';

Object.assign(globalThis, { window: {} });
afterEach(() => clearMocks());

const oauth = { appId: 'app', scope: 'scope', redirectUri: 'https://example.com/callback', state: 'web' };

function mockResponse(body: string, status = 200) {
  let config: any;
  let read = false;
  mockIPC((command, args: any) => {
    switch (command) {
      case 'plugin:http|fetch':
        config = args.clientConfig;
        return 1;
      case 'plugin:http|fetch_send':
        return { status, statusText: 'OK', url: config.url, headers: [], rid: 2 };
      case 'plugin:http|fetch_read_body':
        if (read) return [1];
        read = true;
        return [...new TextEncoder().encode(body), 0];
      default:
        throw new Error(`Unexpected IPC: ${command}`);
    }
  });
  return () => config;
}

test('check-login uses plugin IPC and decodes the double-encoded WeChat response', async () => {
  const payload = { errcode: 0, jsdata: { authorize_uuid: 'uuid' } };
  const request = mockResponse(JSON.stringify(JSON.stringify(payload)));
  assert.deepEqual(await fetchWxLocalCheckLogin(14013, oauth), payload);
  assert.equal(request().url, 'https://localhost.weixin.qq.com:14013/api/check-login');
  assert.equal(new Headers(request().headers).get('origin'), 'https://open.weixin.qq.com');
  assert.equal(request().maxRedirections, 0);
});

test('authorize uses the same plugin transport and sends the detected authorization UUID', async () => {
  const request = mockResponse(JSON.stringify({ errcode: 10050 }));
  assert.equal((await fetchWxLocalAuthorize(13013, oauth, 'uuid', { x: 1, y: 2 })).errcode, 10050);
  const body = JSON.parse(new TextDecoder().decode(new Uint8Array(request().data)));
  assert.equal(request().url, 'https://localhost.weixin.qq.com:13013/api/authorize');
  assert.equal(body.apiname, 'qrconnectfastauthorize');
  assert.equal(body.jsdata.authorize_uuid, 'uuid');
  assert.deepEqual(JSON.parse(body.jsdata.data), { x: 1, y: 2 });
});

test('non-success HTTP responses retain the status and port', async () => {
  mockResponse('unavailable', 503);
  await assert.rejects(fetchWxLocalCheckLogin(14013, oauth), /HTTP 503.*14013/);
});

test('string IPC failures become readable errors', async () => {
  mockIPC(() => Promise.reject('URL not allowed'));
  await assert.rejects(fetchWxLocalCheckLogin(14013, oauth), /URL not allowed/);
});

test('request timeout cancels the native request', async () => {
  let cancel: ((reason: Error) => void) | undefined;
  mockIPC((command) => {
    if (command === 'plugin:http|fetch') return 1;
    if (command === 'plugin:http|fetch_send') {
      return new Promise((_, reject) => { cancel = reject; });
    }
    if (command === 'plugin:http|fetch_cancel') {
      cancel?.(new Error('Request cancelled'));
      return;
    }
    throw new Error(`Unexpected IPC: ${command}`);
  });
  await assert.rejects(fetchWxLocalCheckLogin(14013, oauth, 20), /请求超时/);
});
