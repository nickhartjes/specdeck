// The documentation site, published to GitHub Pages by .github/workflows/docs.yml.
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://nickhartjes.github.io',
  base: '/specdeck',
  integrations: [
    starlight({
      title: 'SpecDeck',
      description: 'Run a coding agent with your team watching along.',
      logo: { src: './src/assets/logo.svg' },
      favicon: '/favicon.svg',
      customCss: ['./src/styles/custom.css'],
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/nickhartjes/specdeck' }],
      editLink: { baseUrl: 'https://github.com/nickhartjes/specdeck/edit/main/docs/site/' },
      lastUpdated: true,
      sidebar: [
        { label: 'Start here', items: ['install', 'guide/first-steps'] },
        {
          label: 'Use SpecDeck',
          items: [
            'guide/host',
            'guide/join',
            'guide/follow',
            'guide/documents',
            'guide/settings',
            'guide/keyboard',
            'guide/troubleshooting',
          ],
        },
        {
          label: 'Run your own server',
          items: [
            'self-host/overview',
            'self-host/docker-compose',
            'self-host/kubernetes',
            'self-host/configuration',
            'self-host/connect-apps',
          ],
        },
      ],
    }),
  ],
});
