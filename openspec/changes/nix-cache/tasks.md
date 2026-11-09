# Tasks

## 1. Flake input

- [x] 1.1 Add `inputs.crane.url = "github:ipetkov/crane"` to `flake.nix`, pass `craneLib = crane.mkLib pkgs` to `nix/package.nix` in `packages`, and `crane.mkLib final` in `overlays.default` (design decision 1); lock it with `nix flake lock` in a `nixos/nix` container, or in CI if the container can't reach GitHub; verify `flake.lock` has a `crane` node and nixpkgs' `rev` is unchanged

## 2. Package

- [x] 2.1 Rewrite `nix/package.nix` on crane: `commonArgs` (src, inputs, `dontUseCmakeConfigure`, `env`, release profile, host target), `cargoArtifacts = craneLib.buildDepsOnly` with both `-p specdeck-app --features tauri/custom-protocol` and `-p specdeck-session` builds, and the package as `mkCargoDerivation` running the Tauri build and install (the hook's functions, or the same commands written out) with `cargoArtifacts` as an attribute of the package (design decision 1); verify locally that `nix eval .#packages.x86_64-linux.specdeck.drvPath` and `.cargoArtifacts.outPath` evaluate in the container
- [x] 2.2 Keep `pnpmDeps` as the package's attribute with the same `fetchPnpmDeps` arguments and the only `hash = "sha256-…"` line in the file; verify with `grep -c 'hash = "sha256-' nix/package.nix` (1) and that `nix eval .#specdeck.pnpmDeps.outputHash` gives the current hash
- [x] 2.3 Keep `pname`, `version`, `signallingUrl`, `postInstall`, `preFixup` and `meta`, and update the file's header comment (crane, why the split); verify by reading the diff against main that only the build machinery changed
- [x] 2.4 Check that the dependency derivation does not depend on the code: change a line in a workspace `.rs` file in the container checkout; verify `.cargoArtifacts.outPath` stays the same and the package's `drvPath` changes

## 3. Workflow

- [x] 3.1 In `.github/workflows/nix.yml`, add the steps from design decision 2: evaluate the dependency path, restore `nix-deps-x86_64-linux-<hash>` (exact key) and `nix copy --no-check-sigs --from` on a hit, build as before, and on a miss on a push to main `nix copy --to file://…?compression=none`, `actions/cache/save` and delete main's other `nix-deps-` entries (`permissions: actions: write`); verify with `actionlint`
- [x] 3.2 Rewrite `nix.yml`'s header comment (dependencies cached by store path, saved from main only; the path filter stays because a `Cargo.lock` change still builds in full) and the CI paragraph in `README.md` (the Nix job's cache; flake inputs nixpkgs and crane are updated by hand); verify the text matches the workflow

## 4. Check in CI (after approval to push the branch)

- [x] 4.1 Ask before pushing `nickhartjes/nix-cache`; then add temporary verification steps (build main's flake as `result-old`, compare file lists, sizes, `.desktop`, icons, `bin/` wrappers and `nix store diff-closures`; let this branch save the cache) and run `nix.yml` by `workflow_dispatch`; verify the outputs differ only in store hashes and the binary starts (run 42594374806: same files, text files equal after masking hashes; binaries +1%; the run-time closure lost GCC, binutils, glibc-dev and linux-headers, about 330 MB)
- [x] 4.2 Run `nix.yml` a second time on the branch; verify the cache is restored, the log has no `Compiling` lines for crates outside the workspace, and note the time and the entry's size (`gh cache list`); fix the dependency build's flags if crates rebuild (run 42599880066: restored in 6 s, only the workspace crates and the vendored `datachannel` compiled, build 4 min 48 s, job 5 min 35 s; entry 640 MB)
- [x] 4.3 Check the pnpm hash workflow's path on the branch: `nix build .#specdeck.pnpmDeps --no-link` succeeds, and with a temporary wrong hash it fails with a `got: sha256-…` line that `renovate-nix-hash.yml`'s `sed` extracts; verify both in the run log, then revert the wrong hash
- [x] 4.4 Verify `nix eval .#packages.aarch64-linux.specdeck.drvPath` evaluates in the run
- [x] 4.5 Remove the temporary steps and the branch's save permission, delete the branch's cache entry (`gh cache delete`), and verify `nix.yml` matches design decision 2 and `actionlint` passes

## 5. After merge

- [ ] 5.1 Verify the first push to main after merge saves one `nix-deps-` entry, and the next push restores it and finishes in a few minutes; check with `gh cache list` that the Rust caches of `ci.yml` are still there, and note any eviction in the issue

## Workflow follow-up

- Close issue #97 with the measured times once 5.1 holds.
- Archive the change.
