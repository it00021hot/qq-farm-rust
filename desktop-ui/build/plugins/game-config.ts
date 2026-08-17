import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Plugin, PreviewServer, ViteDevServer } from 'vite';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const desktopUiRoot = path.resolve(__dirname, '../..');
const workspaceRoot = path.resolve(desktopUiRoot, '..');

/** Align with `qq_farm_core::config::paths::game_config_static_dir`. */
function resolveGameConfigDir(): string {
  const fromEnv = process.env.FARM_GAME_CONFIG_DIR?.trim();
  if (fromEnv && fs.existsSync(fromEnv)) {
    return fromEnv;
  }
  return path.join(workspaceRoot, 'assets', 'game_config');
}

function contentType(filePath: string): string {
  switch (path.extname(filePath).toLowerCase()) {
    case '.png':
      return 'image/png';
    case '.jpg':
    case '.jpeg':
      return 'image/jpeg';
    case '.gif':
      return 'image/gif';
    case '.webp':
      return 'image/webp';
    case '.svg':
      return 'image/svg+xml';
    case '.json':
      return 'application/json';
    default:
      return 'application/octet-stream';
  }
}

function attachGameConfigMiddleware(server: ViteDevServer | PreviewServer, root: string) {
  server.middlewares.use((req: IncomingMessage, res: ServerResponse, next: () => void) => {
    const rawUrl = req.url ?? '';
    if (!rawUrl.startsWith('/game-config/') && rawUrl !== '/game-config') {
      next();
      return;
    }

    const urlPath = decodeURIComponent(rawUrl.split('?')[0] ?? '');
    const rel = urlPath.replace(/^\/game-config\/?/, '');
    if (!rel || rel.includes('..')) {
      res.statusCode = 400;
      res.end('bad path');
      return;
    }

    const filePath = path.resolve(root, rel);
    const rootResolved = path.resolve(root);
    if (!filePath.startsWith(rootResolved + path.sep) && filePath !== rootResolved) {
      res.statusCode = 403;
      res.end('forbidden');
      return;
    }

    if (!fs.existsSync(filePath) || !fs.statSync(filePath).isFile()) {
      res.statusCode = 404;
      res.end('not found');
      return;
    }

    res.setHeader('Content-Type', contentType(filePath));
    res.setHeader('Cache-Control', 'public, max-age=86400');
    fs.createReadStream(filePath).pipe(res);
  });

  server.config.logger.info(`[game-config] serving ${root} at /game-config`);
}

function copyGameConfigIntoDist(root: string, outDir: string) {
  const dest = path.join(outDir, 'game-config');
  fs.mkdirSync(outDir, { recursive: true });
  fs.cpSync(root, dest, { recursive: true });
}

/**
 * Serve `/game-config/*` from the local gameConfig tree.
 *
 * - Vite `serve` / preview: middleware (align Go panel `express.static`).
 * - `vite build`: copy into `dist/game-config` so `cargo tauri dev` (builtin
 *   static server, no Vite) and packaged `tauri://` can load same-origin icons.
 */
export function setupGameConfigStatic(): Plugin {
  const root = resolveGameConfigDir();

  return {
    name: 'qq-farm-game-config-static',
    configureServer(server) {
      attachGameConfigMiddleware(server, root);
    },
    configurePreviewServer(server) {
      attachGameConfigMiddleware(server, root);
    },
    closeBundle() {
      if (!fs.existsSync(root)) {
        this.warn(`[game-config] missing ${root}`);
        return;
      }
      const outDir = path.resolve(desktopUiRoot, 'dist');
      copyGameConfigIntoDist(root, outDir);
      this.info(`[game-config] copied ${root} → ${path.join(outDir, 'game-config')}`);
    }
  };
}
