/** Personal/friend land grid helpers (game Land.json is 4 cols × 6 rows, ids 1–24). */

export const FARM_LAND_COLS = 4;

export type LandFootprint = {
  ids: number[];
  col: number;
  row: number;
  colSpan: number;
  rowSpan: number;
};

export function soilLabel(level: number) {
  const map: Record<number, string> = {
    0: '普通',
    1: '黄土地',
    2: '红土地',
    3: '黑土地',
    4: '金土地',
    5: '紫金土地'
  };
  return map[Number(level) || 0] || '';
}

export function soilLevelClass(level: number) {
  const lv = Math.min(5, Math.max(0, Number(level) || 0));
  return `soil-level-${lv}`;
}

function landCoord(id: number) {
  const safe = Math.max(1, Number(id) || 1);
  return {
    col: ((safe - 1) % FARM_LAND_COLS) + 1,
    row: Math.floor((safe - 1) / FARM_LAND_COLS) + 1
  };
}

/** Bounding box from occupiedLandIds (authoritative), not plantSize alone. */
export function landFootprint(land: Api.Farm.LandRow): LandFootprint {
  const raw = (land.occupiedLandIds || []).map(Number).filter(id => id > 0);
  const ids = raw.length > 0 ? [...new Set(raw)] : [Math.max(1, Number(land.id) || 1)];
  let minCol = FARM_LAND_COLS;
  let maxCol = 1;
  let minRow = 99;
  let maxRow = 1;
  for (const id of ids) {
    const { col, row } = landCoord(id);
    if (col < minCol) minCol = col;
    if (col > maxCol) maxCol = col;
    if (row < minRow) minRow = row;
    if (row > maxRow) maxRow = row;
  }
  return {
    ids,
    col: minCol,
    row: minRow,
    colSpan: Math.max(1, maxCol - minCol + 1),
    rowSpan: Math.max(1, maxRow - minRow + 1)
  };
}

function isMultiCell(land: Api.Farm.LandRow) {
  if (land.occupiedByMaster) return false;
  const fp = landFootprint(land);
  if (fp.ids.length > 1) return true;
  return Math.max(1, Number(land.plantSize) || 1) > 1;
}

/** Hide slave cells covered by a master's occupiedLandIds. */
export function visibleLands(lands: Api.Farm.LandRow[]) {
  const list = lands || [];
  const covered = new Set<number>();
  for (const land of list) {
    if (land.occupiedByMaster || !isMultiCell(land)) continue;
    const masterId = Number(land.id) || 0;
    for (const id of landFootprint(land).ids) {
      if (id !== masterId) covered.add(id);
    }
  }
  return list.filter(land => !land.occupiedByMaster && !covered.has(Number(land.id)));
}

export function landGridStyle(land: Api.Farm.LandRow): Record<string, string> {
  const fp = landFootprint(land);
  // Single tiles: pin to own id slot so holes from merges stay aligned.
  if (!isMultiCell(land)) {
    const { col, row } = landCoord(Number(land.id) || 1);
    return {
      gridColumn: `${col} / span 1`,
      gridRow: `${row} / span 1`
    };
  }
  return {
    gridColumn: `${fp.col} / span ${fp.colSpan}`,
    gridRow: `${fp.row} / span ${fp.rowSpan}`
  };
}

export function landCardClass(land: Api.Farm.LandRow, opts?: { compact?: boolean }) {
  const gap = opts?.compact ? 'gap-6px' : 'gap-8px';
  const pad = opts?.compact ? 'p-10px' : 'p-12px';
  const classes = ['farm-land-card', 'h-full', 'flex-col', gap, pad];
  if (land.status === 'locked') classes.push('land-locked');
  else if (land.status === 'dead') classes.push('land-dead');
  else classes.push(soilLevelClass(land.level));
  if (isMultiCell(land)) {
    const fp = landFootprint(land);
    classes.push('farm-land-merged');
    if (fp.colSpan > 1 || fp.rowSpan > 1) {
      classes.push(`farm-land-span-${Math.max(fp.colSpan, fp.rowSpan)}`);
    }
  }
  return classes;
}

export function landIdLabel(land: Api.Farm.LandRow) {
  if (!isMultiCell(land)) return `#${land.id}`;
  const ids = [...landFootprint(land).ids].sort((a, b) => a - b);
  if (ids.length <= 1) return `#${land.id}`;
  return `#${ids[0]}-${ids[ids.length - 1]}`;
}

export type OwnFarmOperateOp = 'harvest' | 'clear' | 'plant' | 'upgrade' | 'all';

function isActionableLand(land: Api.Farm.LandRow) {
  return Boolean(land.unlocked) && !land.occupiedByMaster && land.status !== 'locked';
}

function landCanHarvest(land: Api.Farm.LandRow) {
  if (!isActionableLand(land)) return false;
  if (land.status === 'harvestable' || land.status === 'harvested') return true;
  return land.status === 'growing' && Number(land.matureInSec || 0) <= 0 && Boolean(land.plantName);
}

function landCanFarm(land: Api.Farm.LandRow) {
  return isActionableLand(land) && Boolean(land.needWater || land.needWeed || land.needBug);
}

function landCanPlant(land: Api.Farm.LandRow) {
  return isActionableLand(land) && (land.status === 'empty' || land.status === 'dead');
}

function landCanUpgrade(land: Api.Farm.LandRow) {
  return !land.occupiedByMaster && Boolean(land.couldUpgrade || land.couldUnlock);
}

/** Own-farm header buttons: only ops that currently have work. */
export function visibleOwnFarmOps(lands: Api.Farm.LandRow[]): Set<OwnFarmOperateOp> {
  const list = visibleLands(lands);
  const harvest = list.some(landCanHarvest);
  const clear = list.some(landCanFarm);
  const plant = list.some(landCanPlant);
  const upgrade = list.some(landCanUpgrade);
  const ops = new Set<OwnFarmOperateOp>();
  if (harvest) ops.add('harvest');
  if (clear) ops.add('clear');
  if (plant) ops.add('plant');
  if (upgrade) ops.add('upgrade');
  if (harvest || clear || plant) ops.add('all');
  return ops;
}

/** Countdown tick: remaining 0 on a growing crop becomes harvestable. */
export function tickMatureLands(lands: Api.Farm.LandRow[]): { lands: Api.Farm.LandRow[]; newlyRipe: number } {
  let newlyRipe = 0;
  const next = lands.map(land => {
    const remaining = Number(land.matureInSec || 0);
    if (remaining <= 0) return land;
    const sec = remaining - 1;
    if (sec <= 0 && land.status === 'growing') {
      newlyRipe += 1;
      return { ...land, matureInSec: 0, status: 'harvestable' };
    }
    return { ...land, matureInSec: sec };
  });
  return { lands: next, newlyRipe };
}
