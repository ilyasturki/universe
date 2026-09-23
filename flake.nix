{
  description = "Universe: a gamepad-first game launcher for Linux (core in Rust, UI in Qt 6 via PySide6, modules and sources; no daemon)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      lib = pkgs.lib;
      version = (fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
      # What build.rs puts behind the version: the flake's source has no .git to ask.
      gitRev = self.shortRev or self.dirtyShortRev or "";

      relOf = path: lib.removePrefix (toString ./. + "/") (toString path);
      junk = [
        "__pycache__"
        ".pytest_cache"
        ".ruff_cache"
      ];

      # Kept out of rustSrc: an edit here leaves core and corePy cached
      pyOnly = [
        "crates/universe-py/tests"
        "crates/universe-py/typings"
      ];
      under = dir: rel: rel == dir || lib.hasPrefix "${dir}/" rel;

      rustSrc = lib.cleanSourceWith {
        src = ./.;
        filter =
          path: type:
          let
            rel = relOf path;
          in
          !(lib.elem (baseNameOf path) junk)
          && !(lib.any (d: under d rel) pyOnly)
          && (
            lib.hasPrefix "crates" rel
            || lib.elem rel [
              "Cargo.toml"
              "Cargo.lock"
              "rustfmt.toml"
            ]
          );
      };

      # The trees a Python check needs plus the root pytest configuration, so one tree's change leaves the others cached
      pySrc =
        dirs:
        lib.cleanSourceWith {
          src = ./.;
          filter =
            path: type:
            let
              rel = relOf path;
              within = dir: under dir rel || lib.hasPrefix "${rel}/" dir;
            in
            lib.cleanSourceFilter path type
            && !(lib.elem (baseNameOf path) junk)
            && (
              lib.elem rel [
                "pyproject.toml"
                "conftest.py"
              ]
              || lib.any within dirs
            );
        };

      lintSrc = lib.cleanSourceWith {
        src = ./.;
        filter =
          path: type:
          let
            rel = relOf path;
          in
          lib.cleanSourceFilter path type
          && !(lib.elem (baseNameOf path) (
            junk
            ++ [
              "target"
              ".venv"
            ]
          ))
          && !(lib.hasPrefix ".dev" rel)
          && rel != ".claude/worktrees";
      };

      core = pkgs.rustPlatform.buildRustPackage {
        pname = "universe-core";
        inherit version;
        src = rustSrc;
        cargoLock.lockFile = ./Cargo.lock;
        cargoBuildFlags = [
          "-p"
          "universe"
        ];
        cargoTestFlags = [
          "-p"
          "universe"
        ];
        env.UNIVERSE_GIT_REV = gitRev;
        # chrono ignores TZDIR, so the zone is given as a file
        preCheck = "export TZ=${pkgs.tzdata}/share/zoneinfo/Europe/Paris";
        nativeBuildInputs = [
          pkgs.pkg-config
          pkgs.installShellFiles
        ];
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
        nativeBuildInputs = with pkgs.rustPlatform; [
          cargoSetupHook
          maturinBuildHook
          pkgs.pkg-config
        ];
        buildInputs = [ pkgs.sqlite ];
        buildAndTestSubdir = "crates/universe-py";
        env.UNIVERSE_GIT_REV = gitRev;
        pythonImportsCheck = [ "universe_core" ];
      };

      treePkg =
        kind: src:
        pkgs.stdenvNoCC.mkDerivation {
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
      moduleRuntime = with pkgs; [
        ffmpeg
        trash-cli
        util-linux
      ];
      sourceRuntime = with pkgs; [ gogdl ];
      runtimePath = lib.makeBinPath (
        moduleRuntime
        ++ sourceRuntime
        ++ [
          pkgs.umu-launcher
          pkgs.systemd
        ]
      );
      modulesDir = "${modulesPkg}/share/universe/modules";
      sourcesDir = "${sourcesPkg}/share/universe/sources";
      qtRuntime = with pkgs.qt6; [
        qtdeclarative
        qt5compat
        qtmultimedia
        qtsvg
        qtimageformats
      ];
      qmlImportPath = lib.concatMapStringsSep ":" (p: "${p}/lib/qt-6/qml") (
        with pkgs.qt6;
        [
          qtdeclarative
          qt5compat
          qtmultimedia
        ]
      );
      # Only wrapQtAppsHook sets this for a built app; the check and the dev shell run the host bare. qtimageformats: webp, which SteamGridDB serves
      qtPluginPath = lib.concatMapStringsSep ":" (p: "${p}/lib/qt-6/plugins") (
        with pkgs.qt6;
        [
          qtsvg
          qtimageformats
          qtmultimedia
        ]
      );
      # What the dev shell and the checks share; conftest.py sets the rest (TZ, locale, the offscreen platform)
      checkEnv = {
        QML2_IMPORT_PATH = qmlImportPath;
        QT_PLUGIN_PATH = qtPluginPath;
        TZDIR = "${pkgs.tzdata}/share/zoneinfo";
      };
      uiPy = ps: [
        ps.pyside6
        ps.pysdl2
        ps.qrcode
        ps.xkbcommon
      ];
      pyEnv = extra: pkgs.python3.withPackages (ps: [ ps.pytest ] ++ extra ps);

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
        # fullscreen runs inside gamescope, whose toplevel hardcodes app_id "gamescope"
        startupWMClass = "gamescope";
      };

      ui = pkgs.python3Packages.buildPythonApplication {
        pname = "universe-ui";
        inherit version;
        pyproject = true;
        src = ./ui;
        build-system = [ pkgs.python3Packages.setuptools ];
        dependencies = uiPy pkgs.python3Packages ++ [ corePy ];
        nativeBuildInputs = [
          pkgs.qt6.wrapQtAppsHook
          pkgs.copyDesktopItems
          pkgs.installShellFiles
          pkgs.scdoc
        ];
        postInstall = ''
          scdoc < universe-ui.1.scd > universe-ui.1
          installManPage universe-ui.1
          install -Dm644 icons/hicolor/scalable/apps/universe-ui.svg $out/share/icons/hicolor/scalable/apps/universe-ui.svg
          install -Dm644 icons/hicolor/symbolic/apps/universe-ui-symbolic.svg $out/share/icons/hicolor/symbolic/apps/universe-ui-symbolic.svg
        '';
        buildInputs = with pkgs.qt6; [
          qtbase
          qtdeclarative
          qt5compat
          qtmultimedia
          qtwayland
          qtsvg
          qtimageformats
        ];
        desktopItems = [ uiDesktopItem ];
        # The hooks and systemd's ExecStopPost need the CLI; a Python process has no argv[0] to find it by.
        preFixup = ''
          qtWrapperArgs+=(--prefix LD_LIBRARY_PATH : ${
            lib.makeLibraryPath [
              pkgs.SDL2
              pkgs.pipewire
            ]
          })
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
        paths = [
          core
          modulesPkg
          sourcesPkg
        ];
        nativeBuildInputs = [ pkgs.makeWrapper ];
        postBuild = ''
          wrapProgram $out/bin/universe --set UNIVERSE_MODULES_PATH "${modulesDir}" --set UNIVERSE_SOURCES_PATH "${sourcesDir}" --prefix PATH : "${runtimePath}"
        '';
        meta.mainProgram = "universe";
      };

      pytestOf =
        {
          name,
          dirs,
          tests ? dirs,
          py ? (ps: [ ]),
          runtime ? [ ],
        }:
        pkgs.stdenvNoCC.mkDerivation {
          inherit name;
          src = pySrc dirs;
          env = checkEnv;
          dontWrapQtApps = true;
          nativeBuildInputs = [ (pyEnv py) ] ++ runtime;
          postPatch = "patchShebangs .";
          buildPhase = ''
            export HOME=$TMPDIR
            python3 -m pytest -q -p no:cacheprovider ${lib.escapeShellArgs tests}
          '';
          installPhase = "touch $out";
        };
      pytestUi = pytestOf {
        name = "universe-pytest-ui";
        dirs = [ "ui" ];
        py = ps: uiPy ps ++ [ corePy ];
        runtime = qtRuntime ++ [
          pkgs.systemd
          pkgs.ffmpeg
        ];
      };
      pytestModules = pytestOf {
        name = "universe-pytest-modules";
        dirs = [ "modules" ];
        runtime = moduleRuntime;
      };
      pytestSources = pytestOf {
        name = "universe-pytest-sources";
        dirs = [ "sources" ];
        runtime = sourceRuntime;
      };
      pytestCorePy = pytestOf {
        name = "universe-pytest-core-py";
        dirs = [
          "crates/universe-py/tests"
          "crates/universe-py/typings"
        ];
        tests = [ "crates/universe-py/tests" ];
        py = ps: [ corePy ];
      };

      # The package's vendored tree, linted instead of built: a lint failure leaves `nix build .#universe` alone
      rustLint = core.overrideAttrs (prev: {
        pname = "universe-rust-lint";
        nativeBuildInputs = prev.nativeBuildInputs ++ [
          pkgs.clippy
          pkgs.rustfmt
          pkgs.python3
        ];
        # The two lines are `tools/lint rust`
        buildPhase = ''
          cargo fmt --check
          cargo clippy --workspace --all-targets --frozen -- -D warnings
        '';
        doCheck = false;
        installPhase = "touch $out";
        postInstall = "";
      });

      lint = pkgs.stdenvNoCC.mkDerivation {
        name = "universe-lint";
        src = lintSrc;
        dontWrapQtApps = true;
        nativeBuildInputs = [
          (pyEnv uiPy)
          pkgs.ruff
          pkgs.pyright
          pkgs.biome
          pkgs.nixfmt
          pkgs.actionlint
          pkgs.shellcheck
          pkgs.qt6.qtdeclarative
        ];
        postPatch = "patchShebangs tools";
        buildPhase = ''
          export HOME=$TMPDIR
          tools/lint tree
        '';
        installPhase = "touch $out";
      };
    in
    {
      packages.${system} = {
        inherit core universe universe-shell-extension;
        universe-core-py = corePy;
        modules = modulesPkg;
        sources = sourcesPkg;
        universe-ui = ui;
        default = universe;
      };

      devShells.${system}.default = pkgs.mkShell {
        packages =
          with pkgs;
          [
            cargo
            rustc
            clippy
            rustfmt
            rust-analyzer
            sccache
            pkg-config
            sqlite
            ruff
            pyright
            biome
            nixfmt
            actionlint
            shellcheck
            maturin
            (pyEnv (ps: uiPy ps ++ [ ps.setuptools ]))
            SDL2
          ]
          ++ qtRuntime
          ++ moduleRuntime
          ++ sourceRuntime;
        env = checkEnv;
        shellHook = ''
          export UNIVERSE_MODULES_PATH="$PWD/modules"
          export UNIVERSE_SOURCES_PATH="$PWD/sources"
          export LD_LIBRARY_PATH="${
            lib.makeLibraryPath [ pkgs.pipewire ]
          }''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          export QT_FORCE_STDERR_LOGGING=1
          export RUSTC_WRAPPER=sccache
        '';
      };

      checks.${system} = {
        core = core;
        lint = lint;
        rust-lint = rustLint;
        pytest-ui = pytestUi;
        pytest-modules = pytestModules;
        pytest-sources = pytestSources;
        pytest-core-py = pytestCorePy;
      };

      nixosModules.default = import ./nix/nixos.nix { gsrPkg = pkgs.gpu-screen-recorder; };
      homeModules.default = import ./nix/home-manager.nix {
        universePkg = universe;
        uiPkg = ui;
        extensionPkg = universe-shell-extension;
      };
    };
}
