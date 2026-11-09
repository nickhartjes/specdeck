# Tasks

## 1. Join page

- [x] 1.1 In `app/ui/src/web/WebApp.tsx`, have `JoinPage` request `web-viewer.json` from its own service once (relative to the page, so a service under a path prefix works), and keep `version` when it is a string; ignore every failure
- [x] 1.2 Render "SpecDeck <version>" below the form as muted secondary text (`field-hint` style), only when a version arrived
- [x] 1.3 Tests in `app/ui/src/web/WebApp.test.tsx`: the version shows when the service gives one; nothing shows for `{"protocol":2}` or a failed request; the form still joins

## 2. Verify

- [x] 2.1 `pnpm --dir app/ui test` and the type check pass
- [x] 2.2 Build the page (`pnpm --dir app/ui build:web`), run the server with it, and check the join page shows the workspace version
