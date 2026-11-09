# Proposal

## Why

The Nix workflow takes 13-21 minutes on every push to main (issue #97), because `nix/package.nix` builds the app and all of its roughly 690 crates in one `rustPlatform.buildRustPackage` derivation, so any source change rebuilds everything. The dependencies change only with `Cargo.lock`, `flake.lock` or the Nix package itself, so they can be built once and reused.

## What Changes

- `nix/package.nix` builds with crane: `buildDepsOnly` compiles the dependencies in their own derivation, and the package builds only the workspace crates and the frontend on top of it (`cargoArtifacts`). The package's output, the Tauri build, the session binary, the `.desktop` file and icons, and the `signallingUrl` override stay the same.
- `flake.nix` gets a second input, `crane`, pinned in `flake.lock` and updated by hand with nixpkgs. Renovate's configuration does not change: its nix manager stays off.
- `nix.yml` keeps the dependency derivation's output in the GitHub Actions cache: only that output (a compressed `target/`), copied to a local file cache with `nix copy` and saved under a key from its store path. Saved from main only, as the Rust caches in `ci.yml`; pull requests restore main's. Superseded entries are deleted so one stays.
- The triggers stay the same: every push to main, pull requests that touch what the Nix build depends on, and `workflow_dispatch`. The comment that says the build cannot use a cache is rewritten.
- `renovate-nix-hash.yml` keeps working unchanged: the pnpm dependencies stay `.#specdeck.pnpmDeps`, with the only `hash = "sha256-…"` line in `nix/package.nix`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None: this changes how the Nix package is built and checked in CI, not what it contains or how the app behaves (`skip_specs`). The Nix install requirement in `native-experience` stays as it is.

## Impact

- `nix/package.nix`, `flake.nix`, `flake.lock`, `.github/workflows/nix.yml`, `README.md` (the CI paragraph).
- New flake input `github:ipetkov/crane`. `nix/package.nix` is no longer a plain nixpkgs-style package, which matters only if it is ever sent to nixpkgs.
- One more GitHub Actions cache entry, 640 MB (measured), next to the Rust caches, which already fill 9.96 of the 10 GB. GitHub evicts the least recently used entries.
- A `Cargo.lock`, `flake.lock` or `nix/package.nix` change, and so each release's version bump, still pays the full build once on main.
- No change for Nix users, apart from a run-time closure about 330 MB smaller (it no longer pulls in GCC and binutils): `nix run github:nickhartjes/specdeck` still builds from source, on x86_64 and aarch64 Linux.
