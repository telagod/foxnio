import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';

export default defineConfig(({ mode }) => {
  const config: any = {
    plugins: [sveltekit()],
  };

  // 只在测试模式下添加测试配置
  if (mode === 'test') {
    config.test = {
      environment: 'jsdom',
      globals: true,
      setupFiles: ['./src/__tests__/setup.ts'],
      include: ['src/**/*.{test,spec}.{js,ts}'],
      coverage: {
        reporter: ['text', 'json', 'html', 'lcov'],
        exclude: ['node_modules/', 'src/__tests__/'],
        reportsDirectory: './coverage',
      },
    };
  }

  return config;
});
