# Design

## Context

See proposal.md - Why. Today `nix/package.nix` is one `rustPlatform.buildRustPackage` with `cargo-tauri.hook`. The hook's build phase runs `cargo tauri build` with `--target <host triple>` and `--config tauri.bundle.conf.json`. Its `beforeBuildCommand` builds the UI with pnpm and then, through `scripts/session-binary.mjs`, runs `cargo build -p specdeck-session --release --target <triple>` (`SESSION_BINARY_TARGET=1`). The hook's install phase takes the binaries, `.desktop` file and icons from the deb bundle. Crates come from `cargoLock.lockFile`, so Cargo needs no hash; the pnpm dependencies are `fetchPnpmDeps` with a hash that `renovate-nix-hash.yml` refreshes by building `.#specdeck.pnpmDeps`, reading the `got:` line and rewriting the single `hash = "sha256-…"` line.

`nix.yml` builds `.#default` on `ubuntu-26.04` (x86_64-linux only) with `cachix/install-nix-action`, then starts the binary under xvfb. The repository is public. Its Actions cache holds 12 entries, 9.96 GB of the 10 GB, mostly the `Swatinem/rust-cache` entries of `ci.yml` and `release.yml`, saved from main only. `flake.lock` pins only nixpkgs, by hand; Renovate's nix manager is off.

Nix is not installed on the development machine. Docker and Colima are, so a `nixos/nix` container can evaluate the flake. Full builds happen in CI, because the public npm registry is blocked on this network.

## Goals / Non-Goals

**Goals:**
- A push to main that does not touch `Cargo.lock`, `flake.lock` or `nix/package.nix` restores the compiled dependencies and builds only the workspace crates and the UI.
- The package Nix users get is the same as before: same binaries, `.desktop` file, icons, wrapper and run-time closure.
- `renovate-nix-hash.yml` keeps working without changes.

**Non-Goals:**
- A binary cache that Nix users substitute from (`nixConfig` substituters in the flake), or caching the finished package.
- Paid services (Cachix, FlakeHub Cache).
- Updating flake inputs with Renovate, building aarch64 in CI, or changing which pull requests run the Nix job.
- Sending the package to nixpkgs.

## Decisions

### 1. crane, with the dependencies in `buildDepsOnly`

`flake.nix` gets `inputs.crane.url = "github:ipetkov/crane"`. crane has no flake inputs of its own, so nothing needs `follows`. `nix/package.nix` takes `craneLib` as an argument, and the flake passes `crane.mkLib pkgs`, or `crane.mkLib final` in the overlay. Both are built from the same `commonArgs`: `src`, `strictDeps`, the vendored crates, all build tools except `pnpmConfigHook` (also the Tauri hook, Node, pnpm and `wrapGAppsHook3`, because they reorder `PKG_CONFIG_PATH` and the C flags, which the `*-sys` crates and bindgen track), the libraries, `dontUseCmakeConfigure`, `CARGO_PROFILE_RELEASE_STRIP=false` (as the Tauri hook sets it), and a `preBuild` that drops stdenv's per-derivation `-frandom-seed` from `BINDGEN_EXTRA_CLANG_ARGS`. Both build with `--target <host triple>` and the release profile. The two are:

- `cargoArtifacts = craneLib.buildDepsOnly (commonArgs // { … })`. crane replaces the workspace sources with dummies, keeps only the manifests and `Cargo.lock`, and so stays the same store path across code changes. Its build command runs the same two Cargo builds the package does, so feature unification matches: the app crate with Tauri's release features (`-p specdeck-app --features tauri/custom-protocol`, the features `cargo tauri build` adds; to be confirmed from the build log), and `-p specdeck-session`. Both use `--release --target <triple> --locked`. The heavy parts, the vendored libdatachannel and OpenSSL, are built here.
- The package is `craneLib.mkCargoDerivation (commonArgs // { inherit cargoArtifacts pnpmDeps; … })`. crane unpacks the artifacts into `target/`, and the build runs `cargo tauri build` as the hook does. crane sets its own `buildPhase` and `installPhase`, so the hook's phases do not run on their own. The build and install commands are the hook's, written out in `nix/package.nix` (`cargo tauri build --bundles deb --target <triple>` with `-- --profile release --offline`, then the deb bundle's `usr/` into `$out`); the hook stays for its fixup, which adds GStreamer settings to the wrapper. The package uses `buildPackage`, so crane also strips references to the vendored crate sources from the binaries. Either way the result must pass the comparison in decision 5.

`pnpmDeps` stays an attribute of the package, built by `fetchPnpmDeps` with the same arguments and the only `hash = "sha256-…"` line in the file, so `.#specdeck.pnpmDeps` and the Renovate workflow's `sed` keep working. `cargoArtifacts` is an attribute of the package too, so CI reads its path as `.#specdeck.cargoArtifacts` (decision 2). `meta`, `pname`, `version`, `signallingUrl`, `postInstall` and `preFixup` stay as they are.

Alternatives:
- **naersk:** the same two-step idea, but less maintained, and it can't run a custom build command as easily as crane.
- **crate2nix / cargo2nix:** a derivation per crate gives the finest caching, but needs a generated `Cargo.nix` that has to be regenerated for every `Cargo.lock` change, which Renovate can't do.
- **Keeping `buildRustPackage` and caching the whole store:** the package changes on every commit, so nothing reusable gets cached.

### 2. Only the dependency output goes in the Actions cache

Of the choices, only the GitHub Actions cache is free and needs no new account. To keep its share small, it holds one store path: the output of `cargoArtifacts`. That is crane's zstd tarball of `target/`, with no store references. It is not the whole `/nix/store` (`cache-nix-action`), and not every path a run builds (Magic Nix Cache). The toolchain, WebKitGTK and the rest still come from cache.nixos.org.

`nix.yml`:
1. `nix eval --raw .#specdeck.cargoArtifacts.outPath` gives the store path; its hash part is the cache key, `nix-deps-x86_64-linux-<hash>`. The path changes exactly when the dependencies have to be rebuilt: `Cargo.lock`, `flake.lock`, the manifests or the deps build in `nix/package.nix`. So the key needs no hand-kept file list, and a stale entry is never used.
2. `actions/cache/restore` with that exact key and no `restore-keys`, because crane can't reuse another dependency set. On a hit, `nix copy --no-check-sigs --from file://$RUNNER_TEMP/nix-deps <path>` puts it in the store, and `nix build .#default -L` skips building it.
3. After the build, on a miss and only on a push to main: `nix copy --to file://$RUNNER_TEMP/nix-deps?compression=none <path>`. It's already zstd, so no second compression. Then `actions/cache/save`.
4. Then delete main's other `nix-deps-` entries (`gh cache list --ref refs/heads/main --key nix-deps-` and `gh cache delete`, with `actions: write`), so only one entry stays. Entries of other refs are left alone.

`--no-check-sigs` is safe here: the cache is written only by this repository's runs on main.

Alternatives: a signed file cache configured as a substituter needs a signing key as a secret for no gain over importing one known path; `cache-nix-action` with `gc-max-store-size` is simpler to set up but saves several GB.

### 3. Saved from main only

As in `ci.yml`: a pull request's cache is visible only to that pull request, and saving from pull requests is what pushed the Rust caches past 10 GB before. A pull request that changes `Cargo.lock` (Renovate, lock-file maintenance) builds the dependencies in full, and so does the push to main after its merge, which saves them. Later pushes restore them. The path filter and `workflow_dispatch` stay. A `workflow_dispatch` run on a branch restores main's entry but does not save, except during the verification in decision 5.

### 4. Flake inputs are updated by hand

`crane` is pinned in `flake.lock` like nixpkgs and updated with `nix flake update`. The README says so. Updating nixpkgs or crane changes the dependency path, so the next main push rebuilds the dependencies once. Renovate's configuration does not change: it updates `Cargo.lock`, and crane vendors from it with no hash to update.

### 5. Checking the work in CI

There is no local Nix, so everything is checked on a branch in CI, after you approve pushing it:
- Locally: `actionlint` for the workflow. Evaluating the flake in a `nixos/nix` container (`nix flake show`, `nix eval .#packages.{x86_64,aarch64}-linux.specdeck.drvPath`) catches eval errors, if the container can reach GitHub for the flake inputs.
- On the branch, by `workflow_dispatch`, with a temporary job or steps removed before merge:
  - **Same package:** build main's flake (`nix build github:nickhartjes/specdeck/<main sha> -o result-old`) next to the new one. Compare the file lists and sizes of both outputs, the `.desktop` file, the icons, the `bin/` wrapper scripts and the run-time closures (`nix path-info -r`, `nix store diff-closures`). Only the store hashes may differ. The xvfb "binary starts" step runs as today.
  - **Cache works:** a first run saves the entry (the save condition temporarily allows this branch), and a second run restores it. Its build log has no `Compiling` lines for crates outside the workspace, and the time is noted. Then the branch entry is deleted.
  - **pnpm hash workflow:** `nix build .#specdeck.pnpmDeps --no-link` succeeds. On a temporary commit with a wrong hash, it fails with a `got: sha256-…` line that the workflow's `sed` expression extracts.
  - **aarch64:** `nix eval .#packages.aarch64-linux.specdeck.drvPath` evaluates. Building it stays a hand check, as in `release-and-verification` task 1.2.

## Risks / Trade-offs

- [The dependency build and the package build use different flags or features, so Cargo rebuilds dependencies anyway] → Same `commonArgs`, profile and target, and the same two `-p` builds. The warm-run log check in decision 5 shows any rebuilt crate; adjust the deps command until there are none.
- [The new entry (640 MB measured) pushes the Actions cache over 10 GB and GitHub evicts an entry: probably the least recently used, `release.yml`'s Rust caches, which are used only at releases] → One entry kept at a time, no double compression, size measured on the branch. If eviction of `ci.yml`'s entries shows up, revisit (for example, trimming `target/` in the deps output) before merging.
- [crane's dummy source still changes on each release, through the version bump in `Cargo.toml` and `Cargo.lock`] → Accepted: one full build per release.
- [A cache entry from a broken build] → It's keyed by its own store path and only saved after the whole job succeeds on main. If needed, delete it with `gh cache delete`.
- [`nix/package.nix` is no longer a nixpkgs-style package] → Accepted. A nixpkgs submission would rewrite it with `buildRustPackage` anyway.

## Migration Plan

1. Implement on `nickhartjes/nix-cache`, check locally (`actionlint`, container eval), then push the branch after you approve, and run the checks in decision 5 by `workflow_dispatch`.
2. Remove the temporary verification steps and open the pull request. Its own Nix run builds in full, since nothing is saved for it.
3. After merge, the first push to main builds the dependencies and saves the entry, and the next push should take a few minutes.
4. Rollback: revert the commit. The leftover `nix-deps-` entry ages out or is deleted with `gh cache delete`.
