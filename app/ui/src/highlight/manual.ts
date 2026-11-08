// Prism highlights every `code` element on the page by itself unless it finds this before it
// loads; the page's Markdown renders such elements, and React owns them. Imported first.
(globalThis as { Prism?: object }).Prism = { manual: true, disableWorkerMessageHandler: true };
