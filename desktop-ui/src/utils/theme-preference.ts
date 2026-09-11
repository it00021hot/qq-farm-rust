export type ThemePreference = 'auto' | 'light' | 'dark';

export function themePreference(value: unknown): ThemePreference {
  return value === 'light' || value === 'dark' ? value : 'auto';
}

export function resolveDarkMode(value: unknown, systemDark: boolean): boolean {
  const preference = themePreference(value);
  return preference === 'auto' ? systemDark : preference === 'dark';
}
