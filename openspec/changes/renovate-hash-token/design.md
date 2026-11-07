# Design

## Context

See proposal.md - Why. `renovate-nix-hash.yml` runs on pushes to `renovate/**` that change the npm manifests or lockfile, rebuilds the pnpm dependencies with Nix, and when the hash in `nix/package.nix` changed, commits it as `github-actions[bot]` and pushes with the checkout's `GITHUB_TOKEN`. It then starts `ci.yml` and `nix.yml` with `gh workflow run`, because a `GITHUB_TOKEN` push starts no workflows of its own. On the pull request those dispatched runs do not count as checks, and the `pull_request` runs for the new commit wait as `action_required` (seen on #7, #11, #13, #14, #19 and #23, created 2026-11-02 and 2026-11-03). The fork approval setting (`first_time_contributors`) does not cover this: the branches are in the repository itself.

## Goals / Non-Goals

**Goals:**
- Renovate pull requests show CI and Nix for their newest commit without anyone approving runs.
- The workflow still works, as today, without the new secret.

**Non-Goals:**
- Changing how the hash is computed, or Renovate's configuration.
- A GitHub App for the repository.

## Decisions

### 1. A fine-grained personal access token, `RENOVATE_NIX_HASH_TOKEN`

Limited to `nickhartjes/specdeck`, with only **Contents: read and write**. A push with it counts as the token owner's, so GitHub runs `pull_request` workflows for the commit normally and links them to the pull request. A GitHub App token would not expire and is not tied to a person, but needs an app, its private key as a secret and a token-minting step; for one workflow in a personal repository a token is simpler. Its expiry (at most a year) is the cost: when it lapses, the push fails and the job says so.

### 2. The token is used for the push only

`actions/checkout` gets `token: ${{ secrets.RENOVATE_NIX_HASH_TOKEN || github.token }}`, which also becomes the credential `git push` uses. The commit keeps `github-actions[bot]` as author and committer, so `gitIgnoredAuthors` in `renovate.json` still makes Renovate treat the branch as its own. The Nix build itself needs no token.

### 3. Without the secret, today's behaviour

The "Start CI and Nix" step runs only when the secret is empty (`if: steps.update.outputs.pushed == 'true' && env.HAS_TOKEN != 'true'`), and the job logs a `::warning::` that the pull request's checks need approval. With the secret, the push starts CI and Nix through `pull_request` and the step is skipped, so they do not run twice.

### 4. No loop

The token's push changes only `nix/package.nix`, which is not in the workflow's `paths`, so the workflow does not start itself again. `ci.yml` and `nix.yml` run on `pull_request`, not on pushes to `renovate/**`, so each runs once per commit.

## Risks / Trade-offs

- [The token can write to every branch, including `main`] → Fine-grained, one repository, contents only; it is available only to this workflow's push step through the checkout. A leaked token is revoked in the account's settings.
- [The token expires and Renovate pull requests go back to unchecked] → The push fails visibly in the workflow; the README says where the token comes from so it can be renewed.
- [Workflows run as the token owner] → Runs and the hash commit's push show the owner as actor; the commit author stays the bot.

## Migration Plan

1. Create the token and add it as `RENOVATE_NIX_HASH_TOKEN`; merge the workflow change.
2. The next Renovate npm update shows CI and Nix on its pull request without approval.
3. Rollback: remove the secret; the workflow falls back to dispatching CI and Nix.
