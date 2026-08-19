#!/usr/bin/env node

import { createHash, randomUUID } from 'node:crypto';
import {
  access,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rename,
  rm,
  stat
} from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';

const CONFIG_FILES = ['ItemInfo.json', 'Plant.json', 'RoleLevel.json', 'Land.json'];
const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const VALID_ONLY = new Set(['config', 'images', 'proto']);

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const defaultRepoRoot = path.resolve(scriptDir, '..');

function usage() {
  return `Usage:
  node tools/sync-from-bot.mjs [--bot-root <path>] [--only config|images|proto] [--apply]

Options:
  --bot-root <path>  qq-farm-bot root (default: sibling ../qq-farm-bot)
  --only <category>  sync one category; default syncs every available category
  --apply            write qq-farm-rust; default is a read-only dry run
  --help             show this help

Image source lookup order:
  <bot>/core/src/gameConfig/seed_images_named
  <bot>/tools/img
`;
}

export function parseArgs(argv, repoRoot = defaultRepoRoot) {
  const options = {
    repoRoot: path.resolve(repoRoot),
    botRoot: path.resolve(repoRoot, '..', 'qq-farm-bot'),
    only: null,
    apply: false,
    help: false
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--apply') {
      options.apply = true;
    } else if (arg === '--help' || arg === '-h') {
      options.help = true;
    } else if (arg === '--bot-root') {
      const value = argv[++index];
      if (!value) throw new Error('--bot-root requires a path');
      options.botRoot = path.resolve(value);
    } else if (arg.startsWith('--bot-root=')) {
      options.botRoot = path.resolve(arg.slice('--bot-root='.length));
    } else if (arg === '--only') {
      const value = argv[++index];
      if (!value) throw new Error('--only requires config, images, or proto');
      options.only = value;
    } else if (arg.startsWith('--only=')) {
      options.only = arg.slice('--only='.length);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  if (options.only && !VALID_ONLY.has(options.only)) {
    throw new Error(`invalid --only value: ${options.only}`);
  }
  return options;
}

async function exists(target) {
  try {
    await access(target);
    return true;
  } catch {
    return false;
  }
}

async function ensureDirectory(target, label) {
  let details;
  try {
    details = await stat(target);
  } catch {
    throw new Error(`${label} does not exist: ${target}`);
  }
  if (!details.isDirectory()) {
    throw new Error(`${label} is not a directory: ${target}`);
  }
}

async function listRelativeFiles(root, accept, prefix = '') {
  const directory = path.join(root, prefix);
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const relative = path.posix.join(prefix.split(path.sep).join(path.posix.sep), entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listRelativeFiles(root, accept, relative)));
    } else if (entry.isFile() && accept(relative)) {
      files.push(relative);
    }
  }
  return files.sort();
}

async function sha256(file) {
  const data = await readFile(file);
  return createHash('sha256').update(data).digest('hex');
}

async function buildManifest(root, relativeFiles) {
  const manifest = new Map();
  for (const relative of relativeFiles) {
    manifest.set(relative, await sha256(path.join(root, relative)));
  }
  return manifest;
}

async function validateJsonFiles(sourceRoot) {
  for (const name of CONFIG_FILES) {
    const file = path.join(sourceRoot, name);
    let parsed;
    try {
      parsed = JSON.parse(await readFile(file, 'utf8'));
    } catch (error) {
      throw new Error(`invalid ${name}: ${error.message}`);
    }
    if (!Array.isArray(parsed) || parsed.length === 0) {
      throw new Error(`${name} must be a non-empty JSON array`);
    }
  }
}

async function validatePngFiles(sourceRoot, files) {
  if (files.length === 0) {
    throw new Error(`no PNG files found: ${sourceRoot}`);
  }
  for (const relative of files) {
    const data = await readFile(path.join(sourceRoot, relative));
    if (data.length < PNG_SIGNATURE.length || !data.subarray(0, PNG_SIGNATURE.length).equals(PNG_SIGNATURE)) {
      throw new Error(`invalid PNG signature: ${relative}`);
    }
  }
}

async function validateProtoFiles(sourceRoot, files) {
  if (files.length === 0) {
    throw new Error(`no .proto files found: ${sourceRoot}`);
  }
  for (const relative of files) {
    const content = await readFile(path.join(sourceRoot, relative), 'utf8');
    if (!/^\s*syntax\s*=\s*["']proto[23]["']\s*;/m.test(content)) {
      throw new Error(`missing protobuf syntax declaration: ${relative}`);
    }
  }
}

async function findImageSource(botRoot) {
  const candidates = [
    path.join(botRoot, 'core', 'src', 'gameConfig', 'seed_images_named'),
    path.join(botRoot, 'tools', 'img')
  ];
  for (const candidate of candidates) {
    if (await exists(candidate)) return candidate;
  }
  return null;
}

async function prepareCategory(category, options) {
  if (category === 'config') {
    const sourceRoot = path.join(options.botRoot, 'core', 'src', 'gameConfig');
    const targetRoot = path.join(options.repoRoot, 'assets', 'game_config');
    await ensureDirectory(sourceRoot, 'bot gameConfig');
    await validateJsonFiles(sourceRoot);
    return {
      category,
      sourceRoot,
      targetRoot,
      files: [...CONFIG_FILES],
      exactDirectory: false
    };
  }

  if (category === 'images') {
    const sourceRoot = await findImageSource(options.botRoot);
    if (!sourceRoot) return null;
    const files = await listRelativeFiles(sourceRoot, relative => relative.toLowerCase().endsWith('.png'));
    await validatePngFiles(sourceRoot, files);
    return {
      category,
      sourceRoot,
      targetRoot: path.join(options.repoRoot, 'assets', 'game_config', 'seed_images_named'),
      files,
      exactDirectory: true
    };
  }

  const sourceRoot = path.join(options.botRoot, 'core', 'src', 'proto');
  const targetRoot = path.join(options.repoRoot, 'proto');
  await ensureDirectory(sourceRoot, 'bot proto');
  const files = await listRelativeFiles(sourceRoot, relative => relative.toLowerCase().endsWith('.proto'));
  await validateProtoFiles(sourceRoot, files);
  return {
    category,
    sourceRoot,
    targetRoot,
    files,
    exactDirectory: true
  };
}

async function targetFilesFor(prepared) {
  if (!(await exists(prepared.targetRoot))) return [];
  if (!prepared.exactDirectory) {
    const present = [];
    for (const name of prepared.files) {
      if (await exists(path.join(prepared.targetRoot, name))) present.push(name);
    }
    return present;
  }
  const extension = prepared.category === 'images' ? '.png' : '.proto';
  return listRelativeFiles(prepared.targetRoot, relative => relative.toLowerCase().endsWith(extension));
}

export async function inspectCategory(prepared) {
  const targetFiles = await targetFilesFor(prepared);
  const sourceManifest = await buildManifest(prepared.sourceRoot, prepared.files);
  const targetManifest = await buildManifest(prepared.targetRoot, targetFiles);
  const added = [];
  const modified = [];
  const unchanged = [];
  const deleted = [];

  for (const relative of prepared.files) {
    const sourceHash = sourceManifest.get(relative);
    const targetHash = targetManifest.get(relative);
    if (!targetHash) added.push({ path: relative, hash: sourceHash });
    else if (targetHash !== sourceHash) modified.push({ path: relative, from: targetHash, to: sourceHash });
    else unchanged.push({ path: relative, hash: sourceHash });
  }
  if (prepared.exactDirectory) {
    for (const relative of targetFiles) {
      if (!sourceManifest.has(relative)) {
        deleted.push({ path: relative, hash: targetManifest.get(relative) });
      }
    }
  }
  return { added, modified, deleted, unchanged };
}

async function copyFiles(sourceRoot, targetRoot, files) {
  for (const relative of files) {
    const destination = path.join(targetRoot, relative);
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(path.join(sourceRoot, relative), destination);
  }
}

async function applyExactDirectory(prepared) {
  const parent = path.dirname(prepared.targetRoot);
  await mkdir(parent, { recursive: true });
  const stageRoot = await mkdtemp(path.join(parent, `.${path.basename(prepared.targetRoot)}.sync-stage-`));
  const backupRoot = `${prepared.targetRoot}.sync-backup-${randomUUID()}`;
  let backedUp = false;
  try {
    await copyFiles(prepared.sourceRoot, stageRoot, prepared.files);
    if (await exists(prepared.targetRoot)) {
      await rename(prepared.targetRoot, backupRoot);
      backedUp = true;
    }
    await rename(stageRoot, prepared.targetRoot);
  } catch (error) {
    await rm(stageRoot, { recursive: true, force: true });
    if (backedUp && !(await exists(prepared.targetRoot))) {
      await rename(backupRoot, prepared.targetRoot);
    }
    throw error;
  }
  if (backedUp) {
    await rm(backupRoot, { recursive: true, force: true });
  }
}

async function applyConfigFiles(prepared) {
  await mkdir(prepared.targetRoot, { recursive: true });
  const token = randomUUID();
  const staged = [];
  const backups = [];
  const installed = [];
  try {
    for (const relative of prepared.files) {
      const destination = path.join(prepared.targetRoot, relative);
      const temp = path.join(prepared.targetRoot, `.${relative}.sync-stage-${token}`);
      await copyFile(path.join(prepared.sourceRoot, relative), temp);
      staged.push({ temp, destination });
    }
    for (const entry of staged) {
      if (await exists(entry.destination)) {
        const backup = `${entry.destination}.sync-backup-${token}`;
        await rename(entry.destination, backup);
        backups.push({ backup, destination: entry.destination });
      }
      await rename(entry.temp, entry.destination);
      installed.push(entry.destination);
    }
  } catch (error) {
    await Promise.all(staged.map(entry => rm(entry.temp, { force: true })));
    await Promise.all(installed.map(destination => rm(destination, { force: true })));
    for (const entry of backups.reverse()) {
      if (await exists(entry.backup)) await rename(entry.backup, entry.destination);
    }
    throw error;
  }
  await Promise.all(backups.map(entry => rm(entry.backup, { force: true })));
}

function shortHash(value) {
  return String(value || '').slice(0, 12);
}

function printDiff(category, diff, logger) {
  logger.log(
    `[${category}] +${diff.added.length} ~${diff.modified.length} -${diff.deleted.length} =${diff.unchanged.length}`
  );
  for (const entry of diff.added) logger.log(`  + ${entry.path} ${shortHash(entry.hash)}`);
  for (const entry of diff.modified) {
    logger.log(`  ~ ${entry.path} ${shortHash(entry.from)} -> ${shortHash(entry.to)}`);
  }
  for (const entry of diff.deleted) logger.log(`  - ${entry.path} ${shortHash(entry.hash)}`);
}

export async function syncFromBot(options, logger = console) {
  await ensureDirectory(options.botRoot, 'qq-farm-bot root');
  await ensureDirectory(options.repoRoot, 'qq-farm-rust root');

  const categories = options.only ? [options.only] : ['config', 'images', 'proto'];
  const results = [];
  for (const category of categories) {
    const prepared = await prepareCategory(category, options);
    if (!prepared) {
      const message =
        'bot images not found; run qq-farm-bot/tools/download-game-images.js or provide its tools/img output';
      if (options.only === 'images') throw new Error(message);
      logger.warn(`[images] skipped: ${message}`);
      results.push({ category, skipped: true });
      continue;
    }
    const diff = await inspectCategory(prepared);
    printDiff(category, diff, logger);
    const changed = diff.added.length + diff.modified.length + diff.deleted.length > 0;
    if (options.apply && changed) {
      if (prepared.exactDirectory) await applyExactDirectory(prepared);
      else await applyConfigFiles(prepared);
      logger.log(`[${category}] applied to ${prepared.targetRoot}`);
    }
    results.push({ category, skipped: false, changed, diff, sourceRoot: prepared.sourceRoot });
  }

  if (!options.apply) logger.log('dry-run only; pass --apply to update qq-farm-rust');
  logger.log('next: cargo check -p qq-farm-core');
  logger.log('next: pnpm -C desktop-ui build');
  return results;
}

async function main() {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.help) {
      process.stdout.write(usage());
      return;
    }
    await syncFromBot(options);
  } catch (error) {
    process.stderr.write(`sync-from-bot: ${error.message}\n`);
    process.exitCode = 1;
  }
}

const entryUrl = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : '';
if (entryUrl === import.meta.url) {
  await main();
}
