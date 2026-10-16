{
  lib,
  rustPlatform,
  pkg-config,
  clang,
  makeBinaryWrapper,
  mold,
  wrapGAppsHook4,
  gtk4,
  gtk4-layer-shell,
  libinput,
  wayland,
  wayland-protocols,
  dbus,
  libappindicator-gtk3,
  libxkbcommon,
  alsa-lib,
  libpulseaudio,
  pipewire,
  libjack2,
}:
rustPlatform.buildRustPackage (final: let
  inherit (lib.fileset) toSource unions;
  inherit (lib) licenses platforms;
in {
  pname = "hibiki";
  version = "0.2.0";

  src = toSource {
    root = ../.;
    fileset = unions [
      ../src
      ../assets
      ../style
      ../benches
      ../Cargo.lock
      ../Cargo.toml
    ];
  };
  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [
    mold
    clang
    pkg-config
    makeBinaryWrapper
    wrapGAppsHook4
  ];

  buildInputs = [
    gtk4
    gtk4-layer-shell
    libinput
    wayland
    wayland-protocols
    dbus
    libappindicator-gtk3
    libxkbcommon
    alsa-lib
    libpulseaudio
    pipewire
    libjack2
  ];

  # Install desktop entry and icon
  postInstall = ''
    install -Dm444 ${../assets/hibiki.desktop} $out/share/applications/hibiki.desktop
    install -Dm444 ${../assets/hibiki.metainfo.xml} $out/share/metainfo/hibiki.metainfo.xml
    install -Dm444 ${../assets/hibiki.svg} $out/share/icons/hicolor/scalable/apps/hibiki.svg
  '';

  # Fix for runtime dependencies
  preFixup = ''
    gappsWrapperArgs+=(
      --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath (final.buildInputs ++ [pipewire libjack2 alsa-lib libpulseaudio])}"
    )
  '';

  meta = {
    description = "Elevating the tactile dialogue. A high-fidelity visual and auditory companion that gives your keystrokes a modern resonance.";
    homepage = "https://github.com/linuxmobile/hibiki";
    license = licenses.mit;
    mainProgram = "hibiki";
    platforms = platforms.unix;
  };
})
