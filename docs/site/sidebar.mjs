// The documentation site's sidebar, shared by astro.config.mjs and scripts/check-links.mjs.
export const sidebar = [
  { label: 'Start here', items: ['install', 'guide/first-steps'] },
  {
    label: 'Use SpecDeck',
    items: [
      'guide/host',
      'guide/join',
      'guide/follow',
      'guide/documents',
      'guide/read-and-estimate',
      'guide/browser',
      'guide/settings',
      'guide/keyboard',
      'guide/troubleshooting',
    ],
  },
  {
    label: 'Security and privacy',
    items: [
      'security/overview',
      'security/data',
      'security/sharing-service',
      'security/claude-browser',
      'security/your-computer',
      'security/known-limits',
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
];
