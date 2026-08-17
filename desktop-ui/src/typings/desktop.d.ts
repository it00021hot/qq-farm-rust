/** Desktop IPC / 页面 model（与 Rust DTO camelCase 对齐） */
declare namespace Desktop {
  interface AccountSummary {
    id: string;
    name: string;
    nick: string;
    platform: string;
    qq: string;
    avatar: string;
    running: boolean;
  }

  interface DesktopSnapshot {
    ready: boolean;
    workerCount: number;
    accountCount: number;
    accounts: AccountSummary[];
  }

  type PlantingStrategy =
    | 'preferred'
    | 'level'
    | 'max_exp'
    | 'max_fertilizer'
    | 'level_up'
    | 'bag_seed'
    | string;

  interface SettingsSummary {
    accountId: string;
    strategy: PlantingStrategy;
    preferredSeed: number;
    farmIntervalSec: number;
    farmMinSec: number;
    farmMaxSec: number;
    automationFarm: boolean;
    automationFriend: boolean;
    automationTask: boolean;
    automationSell: boolean;
  }

  /** Aligns with web Socket.IO envelope `{ type, payload, accountId }`. */
  interface AppEventPayload {
    type: string;
    payload?: unknown;
    accountId?: string | null;
    /** @deprecated use `type` */
    kind?: string;
    accountName?: string | null;
    message?: string | null;
  }
}
