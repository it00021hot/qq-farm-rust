import { convertFileSrc } from '@tauri-apps/api/core';
import { GOLD_BEAN_ITEM_ID } from '@/constants/items';
import { isTauriRuntime } from '@/service/tauri/client';

function catalogRelativePath(path: string): string {
  if (path.startsWith('farmcfg:')) {
    try {
      return new URL(path).pathname.replace(/^\//, '');
    } catch {
      return '';
    }
  }
  const normalized = path.startsWith('/') ? path : `/${path}`;
  return normalized.replace(/^\/game-config\/?/, '').replace(/^\//, '');
}

/**
 * Resolve catalog icon path for game config assets.
 *
 * - Vite dev/preview: same-origin `/game-config/*` (middleware + dist copy).
 * - Tauri webview: platform-specific `farmcfg` URL from `convertFileSrc`.
 */
export function resolveCatalogImage(path?: string | null): string {
  if (!path) return '';
  if (/^https?:\/\//i.test(path) || path.startsWith('data:')) {
    return path.startsWith('//') ? `https:${path}` : path;
  }
  if (path.startsWith('asset:')) {
    return path;
  }

  const rel = catalogRelativePath(path);
  if (!rel) return '';
  if (isTauriRuntime()) {
    return convertFileSrc(`/${rel}`, 'farmcfg');
  }
  return `/game-config/${rel}`;
}

export function formatGrowTime(seconds?: number | null): string {
  if (!seconds || seconds <= 0) return '-';
  if (seconds < 60) return `${seconds}秒`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}分`;
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  return mins > 0 ? `${hours}时${mins}分` : `${hours}时`;
}

export function formatPrice(price?: number | null, priceId?: number | null): string {
  const p = price ?? 0;
  if (priceId === GOLD_BEAN_ITEM_ID) return `${p} 金豆`;
  if (priceId === 1004) return `${p} 钻石`;
  return `${p} 金币`;
}

export const rarityLabelMap: Record<number, string> = {
  0: '普通',
  1: '优秀',
  2: '精良',
  3: '稀有',
  4: '史诗',
  5: '传说'
};

export const priceIdOptions = [
  { label: '金币', value: 0 },
  { label: '金豆豆', value: GOLD_BEAN_ITEM_ID },
  { label: '钻石', value: 1004 }
];

export type GameConfigTab = 'seeds' | 'fruits' | 'items';

export const rarityOptions = [
  { label: '普通', value: 0 },
  { label: '优秀', value: 1 },
  { label: '精良', value: 2 },
  { label: '稀有', value: 3 },
  { label: '史诗', value: 4 },
  { label: '传说', value: 5 }
];

export const seasonOptions = [
  { label: '单季', value: 1 },
  { label: '双季', value: 2 }
];

export const sizeOptions = [
  { label: '1×1（普通作物）', value: 0 },
  { label: '2×2（占地4格）', value: 2 },
  { label: '3×3（占地9格）', value: 3 }
];

export const growPhaseTemplates = [
  { label: '4小时 (6阶段)', value: '种子:2400;发芽:2400;小叶子:2400;大叶子:2400;开花:2400;成熟:0;' },
  { label: '8小时 (6阶段)', value: '种子:4800;发芽:4800;小叶子:4800;大叶子:4800;开花:4800;成熟:0;' },
  { label: '12小时 (6阶段)', value: '种子:7200;发芽:7200;小叶子:7200;大叶子:7200;开花:7200;成熟:0;' },
  { label: '24小时 (6阶段)', value: '种子:14400;发芽:14400;小叶子:14400;大叶子:14400;开花:14400;成熟:0;' }
];
