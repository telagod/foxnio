import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig(({ mode }) => {
  const config: Record<string, unknown> = {
    plugins: [tailwindcss(), sveltekit()],
  };

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
