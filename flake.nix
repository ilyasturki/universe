{
  description = "Universe: a gamepad-first game launcher for Linux (core in Rust, UI in Qt 6 via PySide6, modules; no daemon)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      lib = pkgs.lib;
      version = "0.1.0";

      rustSrc = lib.cleanSourceWith {
        src = ./.;
        filter = path: type:
          let rel = lib.removePrefix (toString ./. + "/") (toString path);
          in lib.hasPrefix "crates" rel || rel == "Cargo.toml" || rel == "Cargo.lock" || rel == "crates";
      };

      core = pkgs.rustPlatform.buildRustPackage {
        pname = "universe-core";
        inherit version;
        src = rustSrc;
        cargoLock.lockFile = ./Cargo.lock;
        cargoBuildFlags = [ "-p" "universe" ];
        cargoTestFlags = [ "-p" "universe" ];
        nativeBuildInputs = [ pkgs.pkg-config ];
        meta.mainProgram = "universe";
      };

      # The core as a Python module (`import universe_core`): what the host links.
      corePy = pkgs.python3Packages.buildPythonPackage {
        pname = "universe-core";
        inherit version;
        pyproject = true;
        src = rustSrc;
        cargoDeps = pkgs.rustPlatform.importCargoLock { lockFile = ./Cargo.lock; };
        nativeBuildInputs = with pkgs.rustPlatform; [ cargoSetupHook maturinBuildHook ];
        buildAndTestSubdir = "crates/universe-py";
        pythonImportsCheck = [ "universe_core" ];
      };

      moduleNames = builtins.filter (n: builtins.pathExists (./modules + "/${n}/module.toml"))
        (builtins.attrNames (builtins.readDir ./modules));

      mkModule = name: pkgs.stdenvNoCC.mkDerivation {
        pname = "universe-module-${name}";
        inherit version;
        src = ./modules + "/${name}";
        nativeBuildInputs = [ pkgs.python3 ];
        buildInputs = [ pkgs.python3 ];
        installPhase = ''
          mkdir -p $out/share/universe/modules/${name}
          cp -r . $out/share/universe/modules/${name}/
          rm -rf $out/share/universe/modules/${name}/tests $out/share/universe/modules/${name}/__pycache__
          patchShebangs $out/share/universe/modules/${name}
        '';
      };

      modulePkgs = lib.genAttrs moduleNames mkModule;

      modulesPkg = pkgs.symlinkJoin {
        name = "universe-modules-${version}";
        paths = builtins.attrValues modulePkgs;
      };

      # No gpu-screen-recorder here: it must match the host's setcap gsr-kms-server (nixos.nix pins that package).
      moduleRuntime = with pkgs; [ gogdl ffmpeg trash-cli util-linux ];

      ui = pkgs.python3Packages.buildPythonApplication {
        pname = "universe-ui";
        inherit version;
        pyproject = true;
        src = ./ui;
        build-system = [ pkgs.python3Packages.setuptools ];
        dependencies = with pkgs.python3Packages; [ pyside6 pysdl2 qrcode corePy ];
        nativeBuildInputs = [ pkgs.qt6.wrapQtAppsHook ];
        buildInputs = with pkgs.qt6; [ qtbase qtdeclarative qt5compat qtmultimedia qtwayland qtsvg ];
        dontWrapQtApps = false;
        # The hooks and systemd's ExecStopPost need the CLI; a Python process has no argv[0] to find it by.
        preFixup = ''
          qtWrapperArgs+=(--prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ pkgs.SDL2 ]})
          qtWrapperArgs+=(--set QT_FORCE_STDERR_LOGGING 1)
          qtWrapperArgs+=(--set UNIVERSE_BIN ${universe}/bin/universe)
          qtWrapperArgs+=(--set UNIVERSE_MODULES_PATH ${modulesPkg}/share/universe/modules)
          qtWrapperArgs+=(--prefix PATH : ${lib.makeBinPath (moduleRuntime ++ [ pkgs.umu-launcher pkgs.systemd ])})
        '';
        postFixup = ''
          for f in $out/bin/*; do wrapQtApp "$f"; done
        '';
        doCheck = false;
      };

      # The core wrapped with the shipped modules and their runtime on PATH: `nix build .#universe`.
      universe = pkgs.symlinkJoin {
        name = "universe-${version}";
        paths = [ core modulesPkg ];
        nativeBuildInputs = [ pkgs.makeWrapper ];
        postBuild = ''
          wrapProgram $out/bin/universe \
            --set UNIVERSE_MODULES_PATH "${modulesPkg}/share/universe/modules" \
            --prefix PATH : "${lib.makeBinPath (moduleRuntime ++ [ pkgs.umu-launcher pkgs.systemd ])}"
        '';
        meta.mainProgram = "universe";
      };

      pytestUi = pkgs.stdenvNoCC.mkDerivation {
        name = "universe-pytest-ui";
        src = ./ui;
        dontWrapQtApps = true;
        nativeBuildInputs = [ (pkgs.python3.withPackages (ps: [ ps.pyside6 ps.pysdl2 ps.qrcode ps.pytest corePy ])) pkgs.qt6.qt5compat pkgs.qt6.qtmultimedia pkgs.qt6.qtdeclarative pkgs.systemd ];
        buildPhase = ''
          export HOME=$TMPDIR QT_QPA_PLATFORM=offscreen QT_FORCE_STDERR_LOGGING=1 LC_ALL=C.UTF-8 TZ=Europe/Paris TZDIR=${pkgs.tzdata}/share/zoneinfo
          export QML2_IMPORT_PATH=${pkgs.qt6.qtdeclarative}/lib/qt-6/qml:${pkgs.qt6.qt5compat}/lib/qt-6/qml:${pkgs.qt6.qtmultimedia}/lib/qt-6/qml
          python3 -m pytest -q -p no:cacheprovider
        '';
        installPhase = "touch $out";
      };

      pytestModules = pkgs.stdenvNoCC.mkDerivation {
        name = "universe-pytest-modules";
        src = ./modules;
        nativeBuildInputs = [ (pkgs.python3.withPackages (ps: [ ps.pytest ])) ] ++ moduleRuntime;
        postPatch = "patchShebangs .";
        buildPhase = ''
          export HOME=$TMPDIR LC_ALL=C.UTF-8 TZ=Europe/Paris TZDIR=${pkgs.tzdata}/share/zoneinfo
          python3 -m pytest -q -p no:cacheprovider
        '';
        installPhase = "touch $out";
      };
    in {
      packages.${system} = {
        inherit core universe;
        universe-core-py = corePy;
        modules = modulesPkg;
        universe-ui = ui;
        default = universe;
      } // lib.mapAttrs' (n: v: lib.nameValuePair "modules-${n}" v) modulePkgs;

      apps.${system} = {
        default = { type = "app"; program = "${universe}/bin/universe"; meta.description = "Universe launcher CLI"; };
        universe-ui = { type = "app"; program = "${ui}/bin/universe-ui"; meta.description = "Universe Qt UI"; };
      };

      overlays.default = final: prev: { universe = universe; universe-ui = ui; universe-core = core; universe-modules = modulesPkg; };

      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [ cargo rustc clippy rustfmt rust-analyzer pkg-config ruff (python3.withPackages (ps: [ ps.pyside6 ps.pysdl2 ps.qrcode ps.pytest ])) qt6.qtdeclarative qt6.qt5compat qt6.qtmultimedia qt6.qtsvg SDL2 ] ++ moduleRuntime;
        shellHook = ''
          export UNIVERSE_MODULES_PATH="$PWD/modules"
          export QML2_IMPORT_PATH="${pkgs.qt6.qtdeclarative}/lib/qt-6/qml:${pkgs.qt6.qt5compat}/lib/qt-6/qml:${pkgs.qt6.qtmultimedia}/lib/qt-6/qml"
          export QT_PLUGIN_PATH="${pkgs.qt6.qtsvg}/lib/qt-6/plugins"
          export QT_FORCE_STDERR_LOGGING=1
        '';
      };

      checks.${system} = {
        core = core;
        pytest-ui = pytestUi;
        pytest-modules = pytestModules;
      };

      nixosModules.default = import ./nix/nixos.nix { universePkg = universe; uiPkg = ui; gsrPkg = pkgs.gpu-screen-recorder; };
      homeModules.default = import ./nix/home-manager.nix { universePkg = universe; uiPkg = ui; };
    };
}
