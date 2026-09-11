{
  description = "Universe: a gamepad-first game launcher for Linux (core in Rust, UI in Qt 6 via PySide6, modules)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      lib = pkgs.lib;
      version = "0.1.0";

      core = pkgs.rustPlatform.buildRustPackage {
        pname = "universe-core";
        inherit version;
        src = lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            let rel = lib.removePrefix (toString ./. + "/") (toString path);
            in lib.hasPrefix "crates" rel || rel == "Cargo.toml" || rel == "Cargo.lock" || rel == "crates";
        };
        cargoLock.lockFile = ./Cargo.lock;
        nativeBuildInputs = [ pkgs.pkg-config ];
        buildInputs = [ ];
        postInstall = ''
          mkdir -p $out/share/dbus-1/services
          cat > $out/share/dbus-1/services/io.github.ilyasturki.Universe.service <<SVC
          [D-BUS Service]
          Name=io.github.ilyasturki.Universe
          Exec=$out/bin/universed
          SVC
        '';
        meta.mainProgram = "universe";
      };

      modulesPkg = pkgs.stdenvNoCC.mkDerivation {
        pname = "universe-modules";
        inherit version;
        src = ./modules;
        nativeBuildInputs = [ pkgs.python3 ];
        buildInputs = [ pkgs.python3 ];
        installPhase = ''
          mkdir -p $out/share/universe/modules
          for m in */; do
            m=''${m%/}
            [ -f "$m/module.toml" ] || continue
            mkdir -p "$out/share/universe/modules/$m"
            cp -r "$m"/. "$out/share/universe/modules/$m/"
            rm -rf "$out/share/universe/modules/$m/tests" "$out/share/universe/modules/$m/__pycache__"
          done
          patchShebangs $out/share/universe/modules
        '';
      };

      # Runtime tools the shipped modules call by name.
      moduleRuntime = with pkgs; [ gpu-screen-recorder gogdl ffmpeg trash-cli util-linux ];

      pythonWithUi = pkgs.python3.withPackages (ps: [ ps.pyside6 ps.pysdl2 ]);

      ui = pkgs.python3Packages.buildPythonApplication {
        pname = "universe-ui";
        inherit version;
        pyproject = true;
        src = ./ui;
        build-system = [ pkgs.python3Packages.setuptools ];
        dependencies = with pkgs.python3Packages; [ pyside6 pysdl2 ];
        nativeBuildInputs = [ pkgs.qt6.wrapQtAppsHook ];
        buildInputs = with pkgs.qt6; [ qtbase qtdeclarative qt5compat qtmultimedia qtwayland qtsvg ];
        dontWrapQtApps = false;
        preFixup = ''
          qtWrapperArgs+=(--prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ pkgs.SDL2 ]})
          qtWrapperArgs+=(--set QT_FORCE_STDERR_LOGGING 1)
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
          for b in universe universed; do
            wrapProgram $out/bin/$b \
              --set UNIVERSE_MODULES_PATH "${modulesPkg}/share/universe/modules" \
              --prefix PATH : "${lib.makeBinPath (moduleRuntime ++ [ pkgs.umu-launcher pkgs.systemd ])}"
          done
          sed -i "s|Exec=.*|Exec=$out/bin/universed|" $out/share/dbus-1/services/io.github.ilyasturki.Universe.service
        '';
        meta.mainProgram = "universe";
      };

      pytestCheck = pkgs.stdenvNoCC.mkDerivation {
        name = "universe-pytest";
        src = ./.;
        nativeBuildInputs = [ (pkgs.python3.withPackages (ps: [ ps.pyside6 ps.pysdl2 ps.pytest ])) pkgs.ffmpeg pkgs.qt6.qt5compat pkgs.qt6.qtmultimedia pkgs.qt6.qtdeclarative ];
        buildPhase = ''
          export HOME=$TMPDIR QT_QPA_PLATFORM=offscreen QT_FORCE_STDERR_LOGGING=1
          export QML2_IMPORT_PATH=${pkgs.qt6.qtdeclarative}/lib/qt-6/qml:${pkgs.qt6.qt5compat}/lib/qt-6/qml:${pkgs.qt6.qtmultimedia}/lib/qt-6/qml
          python3 -m pytest -q -p no:cacheprovider
        '';
        installPhase = "touch $out";
      };
    in {
      packages.${system} = {
        inherit core universe;
        modules = modulesPkg;
        universe-ui = ui;
        default = universe;
      };

      apps.${system} = {
        default = { type = "app"; program = "${universe}/bin/universe"; };
        universed = { type = "app"; program = "${universe}/bin/universed"; };
        universe-ui = { type = "app"; program = "${ui}/bin/universe-ui"; };
      };

      overlays.default = final: prev: { universe = universe; universe-ui = ui; universe-core = core; universe-modules = modulesPkg; };

      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [ cargo rustc clippy rustfmt rust-analyzer pkg-config ruff (python3.withPackages (ps: [ ps.pyside6 ps.pysdl2 ps.pytest ])) qt6.qtdeclarative qt6.qt5compat qt6.qtmultimedia SDL2 ] ++ moduleRuntime;
        shellHook = ''
          export UNIVERSE_MODULES_PATH="$PWD/modules"
          export QML2_IMPORT_PATH="${pkgs.qt6.qtdeclarative}/lib/qt-6/qml:${pkgs.qt6.qt5compat}/lib/qt-6/qml:${pkgs.qt6.qtmultimedia}/lib/qt-6/qml"
          export QT_FORCE_STDERR_LOGGING=1
        '';
      };

      checks.${system} = {
        core = core;
        pytest = pytestCheck;
      };

      nixosModules.default = import ./nix/nixos.nix { universePkg = universe; uiPkg = ui; };
      homeManagerModules.default = import ./nix/home-manager.nix { universePkg = universe; uiPkg = ui; };
    };
}
