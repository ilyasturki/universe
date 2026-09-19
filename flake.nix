{
  description = "Universe: a gamepad-first game launcher for Linux (core in Rust, UI in Qt 6 via PySide6, modules and sources; no daemon)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      lib = pkgs.lib;
      version = (fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
      # What build.rs puts behind the version: the flake's source has no .git to ask.
      gitRev = self.shortRev or self.dirtyShortRev or "";

      rustSrc = lib.cleanSourceWith {
        src = ./.;
        filter = path: type:
          let rel = lib.removePrefix (toString ./. + "/") (toString path);
          in lib.hasPrefix "crates" rel || rel == "Cargo.toml" || rel == "Cargo.lock";
      };

      core = pkgs.rustPlatform.buildRustPackage {
        pname = "universe-core";
        inherit version;
        src = rustSrc;
        cargoLock.lockFile = ./Cargo.lock;
        cargoBuildFlags = [ "-p" "universe" ];
        cargoTestFlags = [ "-p" "universe" ];
        env.UNIVERSE_GIT_REV = gitRev;
        # chrono ignores TZDIR, so the zone is given as a file
        preCheck = "export TZ=${pkgs.tzdata}/share/zoneinfo/Europe/Paris";
        nativeBuildInputs = [ pkgs.pkg-config pkgs.installShellFiles ];
        buildInputs = [ pkgs.sqlite ];
        postInstall = ''
          $out/bin/universe __generate gen
          installShellCompletion --fish gen/universe.fish
          installManPage gen/man/*.1
        '';
        meta.mainProgram = "universe";
      };

      corePy = pkgs.python3Packages.buildPythonPackage {
        pname = "universe-core";
        inherit version;
        pyproject = true;
        src = rustSrc;
        cargoDeps = pkgs.rustPlatform.importCargoLock { lockFile = ./Cargo.lock; };
        nativeBuildInputs = with pkgs.rustPlatform; [ cargoSetupHook maturinBuildHook pkgs.pkg-config ];
        buildInputs = [ pkgs.sqlite ];
        buildAndTestSubdir = "crates/universe-py";
        env.UNIVERSE_GIT_REV = gitRev;
        pythonImportsCheck = [ "universe_core" ];
      };

      treePkg = kind: src: pkgs.stdenvNoCC.mkDerivation {
        pname = "universe-${kind}";
        inherit version src;
        nativeBuildInputs = [ pkgs.python3 ];
        installPhase = ''
          mkdir -p $out/share/universe
          cp -r . $out/share/universe/${kind}
          rm -rf $out/share/universe/${kind}/*/tests $out/share/universe/${kind}/*/extension
          patchShebangs $out/share/universe/${kind}
        '';
      };
      modulesPkg = treePkg "modules" ./modules;
      sourcesPkg = treePkg "sources" ./sources;

      universe-shell-extension = pkgs.stdenvNoCC.mkDerivation (finalAttrs: {
        pname = "universe-shell-extension";
        inherit version;
        src = ./modules/capture/extension;
        installPhase = ''
          runHook preInstall
          install -Dm644 metadata.json extension.js -t \
            "$out/share/gnome-shell/extensions/${finalAttrs.passthru.extensionUuid}"
          runHook postInstall
        '';
        passthru.extensionUuid = "universe@ilyasturki.github.io";
      });

      # No gpu-screen-recorder here: it must match the host's setcap gsr-kms-server (nixos.nix pins that package).
      moduleRuntime = with pkgs; [ ffmpeg trash-cli util-linux ];
      sourceRuntime = with pkgs; [ gogdl ];
      runtimePath = lib.makeBinPath (moduleRuntime ++ sourceRuntime ++ [ pkgs.umu-launcher pkgs.systemd ]);
      modulesDir = "${modulesPkg}/share/universe/modules";
      sourcesDir = "${sourcesPkg}/share/universe/sources";
      qmlImportPath = lib.concatMapStringsSep ":" (p: "${p}/lib/qt-6/qml") (with pkgs.qt6; [ qtdeclarative qt5compat qtmultimedia ]);
      # Only wrapQtAppsHook sets this for a built app; the check and the dev shell run the host bare. qtimageformats: webp, which SteamGridDB serves
      qtPluginPath = lib.concatMapStringsSep ":" (p: "${p}/lib/qt-6/plugins") (with pkgs.qt6; [ qtsvg qtimageformats qtmultimedia ]);

      uiDesktopItem = pkgs.makeDesktopItem {
        name = "universe-ui";
        desktopName = "Universe";
        genericName = "Game Launcher";
        comment = "Gamepad-first game library";
        exec = "universe-ui";
        icon = "universe-ui";
        terminal = false;
        categories = [ "Game" ];
        startupNotify = true;
      };

      ui = pkgs.python3Packages.buildPythonApplication {
        pname = "universe-ui";
        inherit version;
        pyproject = true;
        src = ./ui;
        build-system = [ pkgs.python3Packages.setuptools ];
        dependencies = with pkgs.python3Packages; [ pyside6 pysdl2 qrcode corePy ];
        nativeBuildInputs = [ pkgs.qt6.wrapQtAppsHook pkgs.copyDesktopItems pkgs.installShellFiles pkgs.scdoc ];
        postInstall = ''
          scdoc < universe-ui.1.scd > universe-ui.1
          installManPage universe-ui.1
          install -Dm644 icons/hicolor/scalable/apps/universe-ui.svg $out/share/icons/hicolor/scalable/apps/universe-ui.svg
          install -Dm644 icons/hicolor/symbolic/apps/universe-ui-symbolic.svg $out/share/icons/hicolor/symbolic/apps/universe-ui-symbolic.svg
        '';
        buildInputs = with pkgs.qt6; [ qtbase qtdeclarative qt5compat qtmultimedia qtwayland qtsvg qtimageformats ];
        desktopItems = [ uiDesktopItem ];
        # The hooks and systemd's ExecStopPost need the CLI; a Python process has no argv[0] to find it by.
        preFixup = ''
          qtWrapperArgs+=(--prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ pkgs.SDL2 pkgs.pipewire ]})
          qtWrapperArgs+=(--set QT_FORCE_STDERR_LOGGING 1)
          qtWrapperArgs+=(--set UNIVERSE_BIN ${universe}/bin/universe)
          qtWrapperArgs+=(--set UNIVERSE_MODULES_PATH ${modulesDir})
          qtWrapperArgs+=(--set UNIVERSE_SOURCES_PATH ${sourcesDir})
          qtWrapperArgs+=(--prefix PATH : ${runtimePath})
        '';
        postFixup = ''
          for f in $out/bin/*; do wrapQtApp "$f"; done
        '';
        doCheck = false;
        meta.mainProgram = "universe-ui";
      };

      universe = pkgs.symlinkJoin {
        name = "universe-${version}";
        paths = [ core modulesPkg sourcesPkg ];
        nativeBuildInputs = [ pkgs.makeWrapper ];
        postBuild = ''
          wrapProgram $out/bin/universe --set UNIVERSE_MODULES_PATH "${modulesDir}" --set UNIVERSE_SOURCES_PATH "${sourcesDir}" --prefix PATH : "${runtimePath}"
        '';
        meta.mainProgram = "universe";
      };

      pytestUi = pkgs.stdenvNoCC.mkDerivation {
        name = "universe-pytest-ui";
        src = ./ui;
        dontWrapQtApps = true;
        nativeBuildInputs = [ (pkgs.python3.withPackages (ps: [ ps.pyside6 ps.pysdl2 ps.qrcode ps.pytest corePy ])) pkgs.qt6.qt5compat pkgs.qt6.qtmultimedia pkgs.qt6.qtdeclarative pkgs.qt6.qtsvg pkgs.qt6.qtimageformats pkgs.systemd pkgs.ffmpeg ];
        buildPhase = ''
          export HOME=$TMPDIR QT_QPA_PLATFORM=offscreen QT_FORCE_STDERR_LOGGING=1 LC_ALL=C.UTF-8 TZ=Europe/Paris TZDIR=${pkgs.tzdata}/share/zoneinfo
          export QML2_IMPORT_PATH=${qmlImportPath} QT_PLUGIN_PATH=${qtPluginPath}
          python3 -m pytest -q -p no:cacheprovider
        '';
        installPhase = "touch $out";
      };

      pytestOf = name: src: runtime: pkgs.stdenvNoCC.mkDerivation {
        inherit name src;
        nativeBuildInputs = [ (pkgs.python3.withPackages (ps: [ ps.pytest ])) ] ++ runtime;
        postPatch = "patchShebangs .";
        buildPhase = ''
          export HOME=$TMPDIR LC_ALL=C.UTF-8 TZ=Europe/Paris TZDIR=${pkgs.tzdata}/share/zoneinfo
          python3 -m pytest -q -p no:cacheprovider
        '';
        installPhase = "touch $out";
      };
      pytestModules = pytestOf "universe-pytest-modules" ./modules moduleRuntime;
      pytestSources = pytestOf "universe-pytest-sources" ./sources sourceRuntime;
    in {
      packages.${system} = {
        inherit core universe universe-shell-extension;
        universe-core-py = corePy;
        modules = modulesPkg;
        sources = sourcesPkg;
        universe-ui = ui;
        default = universe;
      };

      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [ cargo rustc clippy rustfmt rust-analyzer pkg-config sqlite ruff maturin (python3.withPackages (ps: [ ps.pyside6 ps.pysdl2 ps.qrcode ps.pytest ps.setuptools ])) qt6.qtdeclarative qt6.qt5compat qt6.qtmultimedia qt6.qtsvg qt6.qtimageformats SDL2 ] ++ moduleRuntime ++ sourceRuntime;
        shellHook = ''
          export UNIVERSE_MODULES_PATH="$PWD/modules"
          export UNIVERSE_SOURCES_PATH="$PWD/sources"
          export QML2_IMPORT_PATH="${qmlImportPath}"
          export QT_PLUGIN_PATH="${qtPluginPath}"
          export LD_LIBRARY_PATH="${lib.makeLibraryPath [ pkgs.pipewire ]}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          export QT_FORCE_STDERR_LOGGING=1
        '';
      };

      checks.${system} = {
        core = core;
        pytest-ui = pytestUi;
        pytest-modules = pytestModules;
        pytest-sources = pytestSources;
      };

      nixosModules.default = import ./nix/nixos.nix { gsrPkg = pkgs.gpu-screen-recorder; };
      homeModules.default = import ./nix/home-manager.nix { universePkg = universe; uiPkg = ui; extensionPkg = universe-shell-extension; };
    };
}
