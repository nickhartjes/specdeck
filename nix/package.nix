# SpecDeck built from source: the UI with pnpm, the app with `cargo tauri build`, whose deb bundle
# gives the .desktop file (with the specdeck:// handler) and the icons.
#
# Built with crane in two derivations: `cargoArtifacts` compiles the dependencies from Cargo.lock
# alone, so it changes only with Cargo.lock, the manifests or this file, and CI caches it
# (.github/workflows/nix.yml); the package builds the workspace crates on top of it. Both build
# with the same target, profile and features, so Cargo reuses the dependencies.
#
# The sharing service is baked in at build time: the public one, as in the release builds. Override
# `signallingUrl` for your team (null leaves it to the app's own default), or set
# SPECDECK_SIGNALLING_URL when starting the app.
{
  lib,
  src,
  craneLib,
  signallingUrl ? "wss://specdeck.hartj.es/signal",
  stdenv,
  rustPlatform,
  cargo-tauri,
  nodejs,
  pnpm,
  fetchPnpmDeps,
  pnpmConfigHook,
  pkg-config,
  wrapGAppsHook3,
  cmake,
  perl,
  openssl,
  webkitgtk_4_1,
  libsoup_3,
  gtk3,
  glib-networking,
  librsvg,
  libayatana-appindicator,
}:

let
  pname = "specdeck";
  version = (lib.importTOML (src + "/Cargo.toml")).workspace.package.version;
  target = stdenv.hostPlatform.rust.rustcTargetSpec;

  # Shared by both derivations: what the Cargo builds see must match, or Cargo rebuilds the
  # dependencies. That includes the order of PKG_CONFIG_PATH and the C flags, which the *-sys crates
  # and bindgen track, so the dependencies get the package's build tools too.
  commonArgs = {
    inherit pname version src;
    strictDeps = true;
    cargoVendorDir = craneLib.vendorCargoDeps { inherit src; };

    nativeBuildInputs = [
      # Only for its fixup, which adds GStreamer and NVIDIA settings to the wrapper; crane's build
      # and install phases replace the hook's.
      cargo-tauri.hook
      nodejs
      pnpm
      pkg-config
      wrapGAppsHook3
      # The vendored libdatachannel and OpenSSL of the datachannel crate.
      cmake
      perl
      rustPlatform.bindgenHook
    ];

    buildInputs = [
      openssl
      webkitgtk_4_1
      libsoup_3
      gtk3
      glib-networking
      librsvg
      # The tray icon; the bundler looks for it, and the app loads it at run time (see preFixup).
      libayatana-appindicator
    ];

    # CMake is only for the vendored libdatachannel, which the Cargo build runs itself.
    dontUseCmakeConfigure = true;

    # The tests need network access and a pty; they run in CI instead.
    doCheck = false;

    # Stripping is left to Nix, as in nixpkgs' Tauri hook. A profile setting, so the dependencies
    # are built with it too.
    env.CARGO_PROFILE_RELEASE_STRIP = "false";

    # bindgen's build script tracks these flags, which hold stdenv's -frandom-seed, taken from each
    # derivation's own output path; it doesn't affect the bindings.
    preBuild = ''
      BINDGEN_EXTRA_CLANG_ARGS=$(sed -E 's/ -frandom-seed=[^ ]+//' <<<"$BINDGEN_EXTRA_CLANG_ARGS")
      export BINDGEN_EXTRA_CLANG_ARGS
    '';
  };

  # The two Cargo builds of the package: the app as `cargo tauri build` builds it, with Tauri's
  # release feature, and the session binary as scripts/session-binary.mjs builds it.
  cargoArtifacts = craneLib.buildDepsOnly (
    commonArgs
    // {
      buildPhaseCargoCommand = ''
        cargoWithProfile build --locked --target ${target} -p specdeck-app --features tauri/custom-protocol
        cargoWithProfile build --locked --target ${target} -p specdeck-app -p specdeck-session \
          --bin specdeck-session --features tauri/custom-protocol
      '';
    }
  );

  pnpmDeps = fetchPnpmDeps {
    inherit pname version src;
    inherit pnpm;
    fetcherVersion = 4;
    # Fewer parallel requests: the release-age check fetches metadata for every package, which
    # breaks on virtualised networks (Colima, some CI) at pnpm's default concurrency.
    pnpmInstallFlags = [ "--network-concurrency=4" ];
    hash = "sha256-xRmmZZAatdtYk78R7F0brVNRzdkfYPBn+9eqzGbhuoU=";
  };
in
craneLib.buildPackage (
  commonArgs
  // {
    # Both are attributes of the package: renovate-nix-hash.yml builds .#specdeck.pnpmDeps, and
    # nix.yml caches .#specdeck.cargoArtifacts.
    inherit cargoArtifacts pnpmDeps;

    nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pnpmConfigHook ];

    env =
      commonArgs.env
      // {
        # Adds the session binary next to the app (scripts/session-binary.mjs), built with --target
        # like the app, so the dependencies compile once.
        SESSION_BINARY_TARGET = "1";
      }
      // lib.optionalAttrs (signallingUrl != null) { SPECDECK_SIGNALLING_URL = signallingUrl; };

    # As nixpkgs' Tauri hook builds and installs it, with the bundle config that adds the session
    # binary.
    buildPhaseCargoCommand = ''
      pushd app/src-tauri
      cargo tauri build --bundles deb --target ${target} --config tauri.bundle.conf.json \
        -- -j "$NIX_BUILD_CORES" --target ${target} --offline --profile release
      popd
    '';

    # Installed from the bundle below, not from Cargo's build log.
    doNotPostBuildInstallCargoBinaries = true;
    installPhaseCommand = ''
      mkdir -p $out
      mv target/${stdenv.hostPlatform.rust.cargoShortTarget}/release/bundle/deb/*/data/usr/* $out/
    '';

    # Hosted sessions run as this binary, which the app looks for next to itself.
    postInstall = ''
      test -x "$out/bin/specdeck-session"
    '';

    # The tray icon loads the indicator library at run time.
    preFixup = ''
      gappsWrapperArgs+=(--prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ libayatana-appindicator ]})
    '';

    meta = {
      description = "Run a coding agent with your team watching along";
      homepage = "https://github.com/nickhartjes/specdeck";
      license = lib.licenses.mit;
      platforms = lib.platforms.linux;
      mainProgram = "specdeck-app";
    };
  }
)
