import { defineConfig } from '@soybeanjs/eslint-config-vue';

export default defineConfig({
  overrides: {
    // Oxfmt is the canonical formatter; its Vue expression indentation differs
    // from vue/html-indent for nested interpolation/ternary expressions.
    'vue/html-indent': 'off',
    'vue/component-name-in-template-casing': [
      'warn',
      'PascalCase',
      {
        registeredComponentsOnly: false,
        ignores: ['/^icon-/']
      }
    ]
  }
});
