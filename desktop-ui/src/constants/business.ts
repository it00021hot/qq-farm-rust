import { $t } from '@/locales';

export const enableStatusRecord: Record<Api.SystemManage.EnableStatus, App.I18n.I18nKey> = {
  1: 'page.system.common.status.enable',
  2: 'page.system.common.status.disable'
};

export const enableStatusOptions: CommonType.Option<Api.SystemManage.EnableStatus, App.I18n.I18nKey>[] = [
  { value: 1, label: 'page.system.common.status.enable' },
  { value: 2, label: 'page.system.common.status.disable' }
];

export function translateNumberOptions<T extends number>(
  options: CommonType.Option<T, App.I18n.I18nKey>[]
): CommonType.Option<T, string>[] {
  return options.map(option => ({
    ...option,
    label: $t(option.label)
  }));
}

export function translateStringOptions<T extends string>(
  options: CommonType.Option<T, App.I18n.I18nKey>[]
): CommonType.Option<T, string>[] {
  return options.map(option => ({
    ...option,
    label: $t(option.label)
  }));
}

/** farm enable status ("1" | "2") */
export const farmEnableStatusRecord: Record<Api.Farm.EnableStatus, App.I18n.I18nKey> = {
  '1': 'page.farm.common.status.enable',
  '2': 'page.farm.common.status.disable'
};

export const farmEnableStatusOptions: CommonType.Option<Api.Farm.EnableStatus, App.I18n.I18nKey>[] = [
  { value: '1', label: 'page.farm.common.status.enable' },
  { value: '2', label: 'page.farm.common.status.disable' }
];

/** farm run status */
export const farmRunStatusRecord: Record<Api.Farm.RunStatus, App.I18n.I18nKey> = {
  0: 'page.farm.common.runStatus.stopped',
  1: 'page.farm.common.runStatus.running',
  2: 'page.farm.common.runStatus.error'
};

export const farmRunStatusOptions: CommonType.Option<Api.Farm.RunStatus, App.I18n.I18nKey>[] = [
  { value: 0, label: 'page.farm.common.runStatus.stopped' },
  { value: 1, label: 'page.farm.common.runStatus.running' },
  { value: 2, label: 'page.farm.common.runStatus.error' }
];

/** fertilizer mode */
export const farmFertilizerModeRecord: Record<Api.Farm.FertilizerMode, App.I18n.I18nKey> = {
  none: 'page.farm.common.fertilizerMode.none',
  normal: 'page.farm.common.fertilizerMode.normal',
  organic: 'page.farm.common.fertilizerMode.organic',
  both: 'page.farm.common.fertilizerMode.both',
  smart: 'page.farm.common.fertilizerMode.smart'
};

export const farmFertilizerModeOptions: CommonType.Option<Api.Farm.FertilizerMode, App.I18n.I18nKey>[] = [
  { value: 'both', label: 'page.farm.common.fertilizerMode.both' },
  { value: 'smart', label: 'page.farm.common.fertilizerMode.smart' },
  { value: 'normal', label: 'page.farm.common.fertilizerMode.normal' },
  { value: 'organic', label: 'page.farm.common.fertilizerMode.organic' },
  { value: 'none', label: 'page.farm.common.fertilizerMode.none' }
];

/** fertilizer land types */
export const farmFertilizerLandTypeOptions: CommonType.Option<string, App.I18n.I18nKey>[] = [
  { value: 'purple-gold', label: 'page.farm.settings.landTypePurpleGold' },
  { value: 'gold', label: 'page.farm.settings.landTypeGold' },
  { value: 'black', label: 'page.farm.settings.landTypeBlack' },
  { value: 'red', label: 'page.farm.settings.landTypeRed' },
  { value: 'normal', label: 'page.farm.settings.landTypeNormal' }
];

export const farmAllFertilizerLandTypes = farmFertilizerLandTypeOptions.map(o => o.value);

/** farm platform */
export const farmPlatformRecord: Record<Api.Farm.Platform, App.I18n.I18nKey> = {
  qq: 'page.farm.common.platform.qq',
  wx: 'page.farm.common.platform.wx'
};

export const farmPlatformOptions: CommonType.Option<Api.Farm.Platform, App.I18n.I18nKey>[] = [
  { value: 'qq', label: 'page.farm.common.platform.qq' },
  { value: 'wx', label: 'page.farm.common.platform.wx' }
];
