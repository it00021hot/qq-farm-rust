import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { parseArgs, syncFromBot } from './sync-from-bot.mjs';

const CONFIG_FILES = ['ItemInfo.json', 'Plant.json', 'RoleLevel.json', 'Land.json'];
const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

async function makeFixture() {
  const root = await mkdtemp(path.join(tmpdir(), 'qq-farm-sync-test-'));
  const botRoot = path.join(root, 'qq-farm-bot');
  const repoRoot = path.join(root, 'qq-farm-rust');
  const gameConfig = path.join(botRoot, 'core', 'src', 'gameConfig');
  const proto = path.join(botRoot, 'core', 'src', 'proto');
  await mkdir(gameConfig, { recursive: true });
  await mkdir(proto, { recursive: true });
  await mkdir(repoRoot, { recursive: true });
  for (const [index, name] of CONFIG_FILES.entries()) {
    await writeFile(path.join(gameConfig, name), `${JSON.stringify([{ id: index + 1, name }], null, 2)}\n`);
  }
  await writeFile(path.join(proto, 'game.proto'), 'syntax = "proto3";\nmessage Game {}\n');
  return { root, botRoot, repoRoot, gameConfig, proto };
}

function silentLogger() {
  return { log() {}, warn() {} };
}

test('parseArgs resolves bot root and validates category', () => {
  const parsed = parseArgs(['--bot-root', '../bot', '--only=proto', '--apply'], '/work/rust');
  assert.equal(parsed.botRoot, path.resolve('../bot'));
  assert.equal(parsed.only, 'proto');
  assert.equal(parsed.apply, true);
  assert.throws(() => parseArgs(['--only', 'unknown'], '/work/rust'), /invalid --only/);
});

test('dry-run reports config changes without writing target', async t => {
  const fixture = await makeFixture();
  t.after(() => rm(fixture.root, { recursive: true, force: true }));
  const target = path.join(fixture.repoRoot, 'assets', 'game_config');
  await mkdir(target, { recursive: true });
  await writeFile(path.join(target, 'ItemInfo.json'), '[{"id":999}]\n');

  const results = await syncFromBot(
    { botRoot: fixture.botRoot, repoRoot: fixture.repoRoot, only: 'config', apply: false },
    silentLogger()
  );

  assert.equal(results[0].diff.modified.length, 1);
  assert.equal(results[0].diff.added.length, 3);
  assert.equal(await readFile(path.join(target, 'ItemInfo.json'), 'utf8'), '[{"id":999}]\n');
});

test('apply mirrors config, downloaded images, and proto while deleting stale files', async t => {
  const fixture = await makeFixture();
  t.after(() => rm(fixture.root, { recursive: true, force: true }));
  const botImages = path.join(fixture.botRoot, 'tools', 'img');
  const targetImages = path.join(fixture.repoRoot, 'assets', 'game_config', 'seed_images_named');
  const targetProto = path.join(fixture.repoRoot, 'proto');
  await mkdir(botImages, { recursive: true });
  await mkdir(targetImages, { recursive: true });
  await mkdir(targetProto, { recursive: true });
  await writeFile(path.join(botImages, '1001.png'), Buffer.concat([PNG_SIGNATURE, Buffer.from('new')]));
  await writeFile(path.join(targetImages, 'stale.png'), Buffer.concat([PNG_SIGNATURE, Buffer.from('old')]));
  await writeFile(path.join(targetProto, 'stale.proto'), 'syntax = "proto3";\nmessage Stale {}\n');
  const sourceBefore = await readFile(path.join(fixture.gameConfig, 'ItemInfo.json'), 'utf8');

  const results = await syncFromBot(
    { botRoot: fixture.botRoot, repoRoot: fixture.repoRoot, only: null, apply: true },
    silentLogger()
  );

  assert.deepEqual(
    results.map(result => result.category),
    ['config', 'images', 'proto']
  );
  assert.equal(await readFile(path.join(fixture.gameConfig, 'ItemInfo.json'), 'utf8'), sourceBefore);
  assert.deepEqual(
    await readFile(path.join(targetImages, '1001.png')),
    Buffer.concat([PNG_SIGNATURE, Buffer.from('new')])
  );
  await assert.rejects(readFile(path.join(targetImages, 'stale.png')), /ENOENT/);
  assert.match(await readFile(path.join(targetProto, 'game.proto'), 'utf8'), /message Game/);
  await assert.rejects(readFile(path.join(targetProto, 'stale.proto')), /ENOENT/);
});

test('invalid source fails before replacing existing config', async t => {
  const fixture = await makeFixture();
  t.after(() => rm(fixture.root, { recursive: true, force: true }));
  const target = path.join(fixture.repoRoot, 'assets', 'game_config');
  await mkdir(target, { recursive: true });
  await writeFile(path.join(target, 'ItemInfo.json'), '[{"id":"keep"}]\n');
  await writeFile(path.join(fixture.gameConfig, 'Plant.json'), '{broken');

  await assert.rejects(
    syncFromBot(
      { botRoot: fixture.botRoot, repoRoot: fixture.repoRoot, only: 'config', apply: true },
      silentLogger()
    ),
    /invalid Plant.json/
  );
  assert.equal(await readFile(path.join(target, 'ItemInfo.json'), 'utf8'), '[{"id":"keep"}]\n');
});

test('explicit image sync fails clearly when bot has no generated images', async t => {
  const fixture = await makeFixture();
  t.after(() => rm(fixture.root, { recursive: true, force: true }));

  await assert.rejects(
    syncFromBot(
      { botRoot: fixture.botRoot, repoRoot: fixture.repoRoot, only: 'images', apply: false },
      silentLogger()
    ),
    /bot images not found/
  );
});
