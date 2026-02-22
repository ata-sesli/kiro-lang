import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: [vitePreprocess()],
	kit: {
		paths: {
            base: process.env.NODE_ENV === 'production' ? '/kiro-lang' : '',
        },
		prerender: {
			handleHttpError: ({ path, referrer, message }) => {
				// Ignore links to .md files from rendered markdown content
				if (path.endsWith('.md')) {
					return;
				}
				throw new Error(message);
			}
		},
		appDir: 'internal',
		adapter: adapter({
			fallback: '404.html'
		}),
	},
	extensions: ['.svelte', '.svx']
};

export default config;
