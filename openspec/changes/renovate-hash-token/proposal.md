# Proposal

## Why

Renovate pull requests that change npm dependencies show no CI. `renovate-nix-hash.yml` commits the updated Nix hash with `GITHUB_TOKEN`; GitHub parks the pull request's CI and Nix runs for that commit as "action_required", and the runs the workflow starts itself (`gh workflow run`) pass but are not linked to the pull request. So six Renovate pull requests looked unchecked until their runs were approved by hand, and every new one will too.

## What Changes

- `renovate-nix-hash.yml` pushes its hash commit with a token of its own, a fine-grained personal access token limited to this repository, instead of `GITHUB_TOKEN`. GitHub then runs the pull request's CI and Nix for that commit as for any other push, and shows them on the pull request.
- The step that starts CI and Nix by hand goes away when the token is set. While the secret is missing, the workflow keeps today's behaviour and logs a warning, as the release workflow does for its optional secrets.
- The commit stays authored by `github-actions[bot]`, so Renovate keeps ignoring it (`gitIgnoredAuthors` in `renovate.json`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None: this changes CI tooling only, no behaviour of the app (`skip_specs`).

## Impact

- `.github/workflows/renovate-nix-hash.yml`.
- New repository secret `RENOVATE_NIX_HASH_TOKEN`; the token expires and has to be renewed.
- `README.md`, where the workflows and secrets are described, if it mentions this workflow.
