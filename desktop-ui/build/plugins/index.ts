import type { PluginOption } from 'vite';
import vue from '@vitejs/plugin-vue';
import vueJsx from '@vitejs/plugin-vue-jsx';
import progress from 'vite-plugin-progress';
import vueRootValidator from 'vite-plugin-vue-transition-root-validator';
import { setupElegantRouter } from './router.ts';
import { setupUnocss } from './unocss.ts';
import { setupUnplugin } from './unplugin.ts';
import { setupHtmlPlugin } from './html.ts';
import { setupDevtoolsPlugin } from './devtools.ts';
import { setupGameConfigStatic } from './game-config.ts';

export function setupVitePlugins(viteEnv: Env.ImportMeta, buildTime: string) {
  const plugins: PluginOption = [
    vue(),
    vueJsx(),
    setupDevtoolsPlugin(viteEnv),
    setupElegantRouter(),
    setupUnocss(viteEnv),
    ...setupUnplugin(viteEnv),
    progress(),
    setupHtmlPlugin(buildTime),
    vueRootValidator(),
    setupGameConfigStatic()
  ];

  return plugins;
}
