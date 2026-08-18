<template>
  <div class="app-logo">
    <svg width="100%" height="100%" viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <defs>
        <linearGradient id="farmLeafA" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stop-color="var(--logo-color-300)" />
          <stop offset="100%" stop-color="var(--logo-color-600)" />
        </linearGradient>
        <linearGradient id="farmLeafB" x1="100%" y1="0%" x2="0%" y2="100%">
          <stop offset="0%" stop-color="var(--logo-color-400)" />
          <stop offset="100%" stop-color="var(--logo-color-700)" />
        </linearGradient>
        <linearGradient id="farmSoil" x1="0%" y1="0%" x2="0%" y2="100%">
          <stop offset="0%" stop-color="#c4a574" />
          <stop offset="100%" stop-color="#8b6914" />
        </linearGradient>
        <linearGradient id="farmBadge" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stop-color="#dea584" />
          <stop offset="100%" stop-color="#9a3412" />
        </linearGradient>
      </defs>

      <ellipse cx="32" cy="52" rx="22" ry="7" fill="url(#farmSoil)" opacity="0.95" />
      <path d="M12 50c4-4 10-6 20-6s16 2 20 6c-3 3-10 5-20 5s-17-2-20-5z" fill="url(#farmSoil)" />

      <path d="M32 48V24" stroke="var(--logo-color-700)" stroke-width="3" stroke-linecap="round" fill="none" />
      <path d="M31 34c-8-1-14-7-15-14 8 1 15 6 16 13-0.2 0.5-0.6 0.9-1 1z" fill="url(#farmLeafA)" />
      <path d="M33 30c8-2 13-8 14-15-8 2-14 8-15 14 0.2 0.5 0.6 0.9 1 1z" fill="url(#farmLeafB)" />
      <path d="M32 24c-4-6-3-12 0-16 3 4 4 10 0 16z" fill="url(#farmLeafA)" />

      <path d="M50 4.2l7.8 4.5v9l-7.8 4.5-7.8-4.5v-9z" fill="url(#farmBadge)" />
      <path
        fill="#fff"
        d="M46.9 9.3h4.2c1.95 0 3.25 1.15 3.25 2.75 0 1.2-.7 2.15-1.85 2.5l2.2 3.65h-2.45l-1.95-3.35h-1.2v3.35h-2.2V9.3zm2.2 4.4h1.85c.85 0 1.35-.5 1.35-1.2s-.5-1.2-1.35-1.2H49.1v2.4z"
      />
    </svg>
  </div>
</template>

<style scoped>
.app-logo {
  --logo-color-300: rgb(var(--primary-300-color));
  --logo-color-400: rgb(var(--primary-400-color));
  --logo-color-500: rgb(var(--primary-500-color));
  --logo-color-600: rgb(var(--primary-600-color));
  --logo-color-700: rgb(var(--primary-700-color));
}
</style>
