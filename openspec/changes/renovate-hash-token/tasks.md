# Tasks

## 1. Token

- [x] 1.1 Create a fine-grained personal access token for `nickhartjes/specdeck` only, with Contents read and write and nothing else, and add it as repository secret `RENOVATE_NIX_HASH_TOKEN` (design decision 1); verify `gh secret list` shows it

## 2. Workflow

- [x] 2.1 In `.github/workflows/renovate-nix-hash.yml`, check out with `token: ${{ secrets.RENOVATE_NIX_HASH_TOKEN || github.token }}`, run the dispatch step only when the secret is missing and log a `::warning::` then (design decisions 2 and 3), and update the header comment; verify with `actionlint` that it parses and that the commit author is still `github-actions[bot]`
- [x] 2.2 Update the Renovate paragraph in `README.md` (the hash commit starts CI and Nix on the pull request; without `RENOVATE_NIX_HASH_TOKEN` it starts them by hand and the pull request's checks need approval); verify the text matches the workflow

## 3. Check

- [ ] 3.1 On the next Renovate pull request that changes npm dependencies (or by re-running the workflow on an open one), verify the hash commit is pushed, CI and Nix appear on the pull request for that commit without approval, they run once, and the workflow does not start itself again
