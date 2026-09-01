/** 生涯统计（收获 / 偷菜）展示格式化。 */

/** ≥10000 显示「X.X万」（最多 2 位小数，去尾零）。 */
export function formatCareerCount(value: unknown): string {
  const num = Number(value) || 0;
  if (num < 10000) return String(num);
  const wan = (num / 10000).toFixed(2).replace(/\.?0+$/, '');
  return `${wan}万`;
}

/** 收偷比 = steal / harvest 百分比（1 位小数）；harvest≤0 且 steal>0 显示 --；双 0 显示 0%。 */
export function formatCareerStealRatio(career?: Api.Farm.Career | null): string {
  const harvest = Number(career?.harvest ?? 0) || 0;
  const steal = Number(career?.steal ?? 0) || 0;
  if (harvest <= 0 && steal > 0) return '--';
  if (harvest <= 0) return '0%';
  return `${((steal / harvest) * 100).toFixed(1)}%`;
}
