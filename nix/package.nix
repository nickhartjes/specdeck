# SpecDeck built from source: the UI with pnpm, the app with Cargo through the Tauri hook, which
# also installs the .desktop file (with the specdeck:// handler) and the icons.
#
# The sharing service is baked in at build time: the public one, as in the release builds. Override
# `signallingUrl` for your team (null leaves it to the app's own default), or set
# SPECDECK_SIGNALLING_URL when starting the app.
{
  lib,
  src,
  signallingUrl ? "wss://specdeck.hartj.es/signal",
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

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "specdeck";
  version = (lib.importTOML (src + "/Cargo.toml")).workspace.package.version;
  inherit src;

  cargoLock.lockFile = src + "/Cargo.lock";

  pnpmDeps = fetchPnpmDeps {
    inherit (finalAttrs) pname version src;
    inherit pnpm;
    fetcherVersion = 4;
    # Fewer parallel requests: the release-age check fetches metadata for every package, which
    # breaks on virtualised networks (Colima, some CI) at pnpm's default concurrency.
    pnpmInstallFlags = [ "--network-concurrency=4" ];
    hash = "sha256-+C6zqGXL2AXqVZ+IschKtYhN0bcb0BdIUf6k1dDfdWk=";
  };

  nativeBuildInputs = [
    cargo-tauri.hook
    nodejs
    pnpm
    pnpmConfigHook
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

  buildAndTestSubdir = "app/src-tauri";

  env = lib.optionalAttrs (signallingUrl != null) { SPECDECK_SIGNALLING_URL = signallingUrl; };

  # The tests need network access and a pty; they run in CI instead.
  doCheck = false;

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
})
