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
            # the components tests read the tools each shipped source pins
            || (
              under "sources" rel
              && (
                type == "directory" && lib.length (lib.splitString "/" rel) <= 2 || baseNameOf path == "source.toml"
              )
            )
            || lib.elem rel [
              "Cargo.toml"
              "Cargo.lock"
              "rustfmt.toml"
              # the core embeds the changelog for the apps' About and what's-new pages
              "CHANGELOG.md"
              # the core embeds the shell extension for `universe setup`
              "extension"
              "extension/extension.js"
              "extension/metadata.json"
              # the install helper's tests read the polkit policy that names it
              "packaging"
              "packaging/system"
              "packaging/system/io.github.ilyasturki.universe.policy"
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
        # reqwest's platform verifier refuses to build a client on an empty CA store; the NetworkManager and BlueZ tests run their own dbus-daemon, the volume keys' test its own PipeWire
        nativeCheckInputs = [
          pkgs.cacert
          pkgs.dbus
          pkgs.pipewire
          pkgs.wireplumber
        ]
        ++ coreRuntime;
        nativeBuildInputs = [
          pkgs.pkg-config
          pkgs.installShellFiles
        ];
        # The helper installs from the distribution's packages, which NixOS gives no system tool.
        postInstall = ''
          rm $out/bin/universe-system-install
          $out/bin/universe __generate gen
          installShellCompletion --cmd universe --fish gen/universe.fish --bash gen/universe.bash --zsh gen/_universe
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
            rm -rf $out/share/universe/${kind}/*/tests $out/share/universe/${kind}/conftest.py $out/share/universe/${kind}/test_*.py
            patchShebangs $out/share/universe/${kind}
          '';
        };
      modulesPkg = (treePkg "modules" ./modules).overrideAttrs {
        postFixup = ''
          substituteInPlace $out/share/universe/modules/pads/bin/_sdl.py $out/share/universe/modules/controls/bin/_gamepads.py \
            --replace-fail '"@libSDL3@"' '"${lib.getLib pkgs.sdl3}/lib/libSDL3.so.0"'
        '';
      };
      # comet's release, not nixpkgs' comet-gog: that package ships no Windows side. tools/dist and the source PKGBUILDs pin the same file
      galaxyServiceStub = pkgs.fetchurl {
        url = "https://github.com/imLinguin/comet/releases/download/v0.3.2/GalaxyCommunication-dummy.exe";
        hash = "sha256-x2lSZ9o2OoYa+Z25XK/mi3Mq50PlgwtP7qG8fudF+Z0=";
      };
      sourcesPkg = (treePkg "sources" ./sources).overrideAttrs (old: {
        installPhase = old.installPhase + ''
          install -Dm644 ${galaxyServiceStub} $out/share/universe/sources/gog/GalaxyCommunication.exe
        '';
      });

      universe-shell-extension = pkgs.stdenvNoCC.mkDerivation (finalAttrs: {
        pname = "universe-shell-extension";
        inherit version;
        src = ./extension;
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
      # The saves engine: on PATH it wins over the build Universe fetches, which NixOS would run in universe-fhs.
      coreRuntime = [ pkgs.ludusavi ];
      sourceRuntime = with pkgs; [
        gogdl
        comet-gog
        legendary-gl
        butler
      ];
      universeFhs = pkgs.buildFHSEnv (
        pkgs.appimageTools.defaultFhsEnvArgs
        // {
          name = "universe-fhs";
          # Host libraries catalogue AppImages expect beyond appimageTools' list: flycast libpng16, snes9x glibmm and libXv.
          targetPkgs =
            p:
            pkgs.appimageTools.defaultFhsEnvArgs.targetPkgs p
            ++ [
              p.libpng
              (p.glibmm_2_4 or p.glibmm)
              p.libxv
            ];
          runScript = pkgs.writeShellScript "universe-fhs-run" ''exec "$@"'';
        }
      );
      runtimePath = lib.makeBinPath (
        moduleRuntime
        ++ sourceRuntime
        ++ coreRuntime
        ++ [
          pkgs.umu-launcher
          pkgs.systemd
          universeFhs
        ]
      );
      modulesDir = "${modulesPkg}/share/universe/modules";
      sourcesDir = "${sourcesPkg}/share/universe/sources";
      # The GTK frontend: what its build script runs, what it links, and what plays its recordings.
      desktopTools = with pkgs; [
        pkg-config
        blueprint-compiler
        glib
        gettext
        desktop-file-utils
        appstream
      ];
      desktopLibs = with pkgs; [
        gtk4
        libadwaita
      ];
      gstPlugins = with pkgs.gst_all_1; [
        gstreamer
        gst-plugins-base
        gst-plugins-good
        gst-plugins-bad
        gst-libav
      ];
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

      ui = pkgs.python3Packages.buildPythonApplication {
        pname = "universe-ui";
        inherit version;
        pyproject = true;
        src = ./ui;
        build-system = [ pkgs.python3Packages.setuptools ];
        dependencies = uiPy pkgs.python3Packages ++ [ corePy ];
        nativeBuildInputs = [
          pkgs.qt6.wrapQtAppsHook
          pkgs.installShellFiles
          pkgs.scdoc
        ];
        postInstall = ''
          scdoc < universe-ui.1.scd > universe-ui.1
          installManPage universe-ui.1
          install -Dm644 universe-ui.desktop -t $out/share/applications
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
        # Steam preloads its overlay into a non-Steam shortcut: the Qt wrapper goes behind a static entry point that drops it (nix/steam-safe.c).
        postFixup = ''
          for f in $out/bin/*; do wrapQtApp "$f"; done
          mv $out/bin/universe-ui $out/bin/.universe-ui-qt
          ${pkgs.pkgsStatic.stdenv.cc}/bin/${pkgs.pkgsStatic.stdenv.cc.targetPrefix}cc -static -O2 \
            -DTARGET='"'"$out/bin/.universe-ui-qt"'"' -o $out/bin/universe-ui ${./nix/steam-safe.c}
        '';
        doCheck = false;
        meta.mainProgram = "universe-ui";
      };

      desktop = pkgs.rustPlatform.buildRustPackage {
        pname = "universe-desktop";
        inherit version;
        src = rustSrc;
        cargoLock.lockFile = ./Cargo.lock;
        cargoBuildFlags = [
          "-p"
          "universe-desktop"
        ];
        cargoTestFlags = [
          "-p"
          "universe-desktop"
        ];
        env = {
          UNIVERSE_GIT_REV = gitRev;
          UNIVERSE_DESKTOP_LOCALEDIR = "${placeholder "out"}/share/locale";
        };
        nativeBuildInputs = desktopTools ++ [
          pkgs.wrapGAppsHook4
          pkgs.scdoc
        ];
        buildInputs = desktopLibs ++ gstPlugins;
        # D-Bus activation runs the .service's Exec with the bus's environment: it has to be the wrapper, by its path.
        postInstall = ''
          data=crates/universe-desktop/data
          id=io.github.ilyasturki.UniverseDesktop
          install -Dm644 $data/$id.desktop -t $out/share/applications
          install -Dm644 $data/$id.service -t $out/share/dbus-1/services
          substituteInPlace $out/share/applications/$id.desktop $out/share/dbus-1/services/$id.service \
            --replace-fail 'Exec=universe-desktop' "Exec=$out/bin/universe-desktop"
          install -Dm644 $data/$id.search-provider.ini -t $out/share/gnome-shell/search-providers
          install -Dm644 $data/$id.metainfo.xml -t $out/share/metainfo
          install -Dm644 $data/icons/apps/$id.svg -t $out/share/icons/hicolor/scalable/apps
          install -Dm644 $data/icons/apps/$id-symbolic.svg -t $out/share/icons/hicolor/symbolic/apps
          mkdir -p $out/share/man/man1
          scdoc < crates/universe-desktop/universe-desktop.1.scd > $out/share/man/man1/universe-desktop.1
          desktop-file-validate $out/share/applications/$id.desktop
          appstreamcli validate --no-net $out/share/metainfo/$id.metainfo.xml
        '';
        # The hooks and systemd's ExecStopPost call the CLI back; the games run the runtime tools off PATH.
        preFixup = ''
          gappsWrapperArgs+=(--set UNIVERSE_BIN ${universe}/bin/universe)
          gappsWrapperArgs+=(--set UNIVERSE_MODULES_PATH ${modulesDir})
          gappsWrapperArgs+=(--set UNIVERSE_SOURCES_PATH ${sourcesDir})
          gappsWrapperArgs+=(--prefix PATH : ${runtimePath})
        '';
        meta.mainProgram = "universe-desktop";
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

      desktopTest =
        pkgs.runCommand "universe-desktop-test"
          {
            nativeBuildInputs = with pkgs; [
              weston
              dbus
              glib
              python3
            ];
            env = {
              UNIVERSE_DESKTOP = lib.getExe desktop;
              UNIVERSE_BIN = lib.getExe universe;
              FONTCONFIG_FILE = pkgs.makeFontsConf { fontDirectories = [ pkgs.cantarell-fonts ]; };
              GSK_RENDERER = "cairo";
              XDG_DATA_DIRS = "${pkgs.adwaita-icon-theme}/share";
            };
          }
          ''
            export HOME=$TMPDIR XDG_RUNTIME_DIR=$TMPDIR/runtime
            mkdir -m 700 $XDG_RUNTIME_DIR
            DESKTOP_TEST_OUT=$out bash ${./tools/desktop-test}
          '';

      pytestOf =
        {
          name,
          dirs,
          tests ? dirs,
          py ? (ps: [ ]),
          runtime ? [ ],
          env ? { },
        }:
        pkgs.stdenvNoCC.mkDerivation {
          inherit name;
          src = pySrc dirs;
          env =
            checkEnv
            // {
              UNIVERSE_TEST_BUDGET_S = "0";
            }
            // env;
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
        # the core contract test lists the modules and sources the real core reads; the theme tests install examples/theme
        dirs = [
          "ui"
          "modules"
          "sources"
          "examples"
        ];
        tests = [ "ui" ];
        py =
          ps:
          uiPy ps
          ++ [
            corePy
            ps.pytest-qt
          ];
        runtime = qtRuntime ++ [
          pkgs.systemd
          pkgs.ffmpeg
        ];
      };
      pytestModules = pytestOf {
        name = "universe-pytest-modules";
        dirs = [
          "modules"
          "extension"
          "crates/universe/src/desktop/gnome.rs"
        ];
        tests = [
          "modules"
          "extension"
        ];
        runtime = moduleRuntime;
        # The controls hook's launch test reads the game and its settings off the real core.
        env.UNIVERSE_BIN = lib.getExe core;
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
        nativeBuildInputs =
          prev.nativeBuildInputs
          ++ desktopTools
          ++ [
            pkgs.clippy
            pkgs.rustfmt
            pkgs.python3
          ];
        buildInputs = desktopLibs ++ gstPlugins;
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
        universe-fhs = universeFhs;
        universe-desktop = desktop;
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
            (pyEnv (
              ps:
              uiPy ps
              ++ [
                ps.setuptools
                ps.pytest-qt
                ps.pytest-xdist
              ]
            ))
            SDL2
            sway
            wf-recorder
            dbus
            pipewire
            wireplumber
          ]
          ++ qtRuntime
          ++ moduleRuntime
          ++ sourceRuntime
          ++ coreRuntime
          ++ desktopTools
          # a headless compositor for `just desktop-shot`
          ++ [ weston ];
        buildInputs = desktopLibs ++ gstPlugins;
        env = checkEnv;
        shellHook = ''
          export UNIVERSE_MODULES_PATH="$PWD/modules"
          export UNIVERSE_SOURCES_PATH="$PWD/sources"
          export LD_LIBRARY_PATH="${
            lib.makeLibraryPath [
              pkgs.pipewire
              pkgs.sdl3
            ]
          }''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          export QT_FORCE_STDERR_LOGGING=1
          export RUSTC_WRAPPER=sccache
          export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}''${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
        '';
      };

      checks.${system} = {
        core = core;
        desktop = desktop;
        desktop-test = desktopTest;
        lint = lint;
        rust-lint = rustLint;
        pytest-ui = pytestUi;
        pytest-modules = pytestModules;
        pytest-sources = pytestSources;
        pytest-core-py = pytestCorePy;
        # A stand-in for Steam's overlay with a dependency nothing resolves: it kills a dynamic binary, not universe-ui.
        steam-preload = pkgs.runCommandCC "universe-ui-steam-preload" { } ''
          mkdir gone overlay
          echo 'void gone(void) {}' > gone.c
          cc -shared -o gone/libgone.so gone.c
          echo 'void gone(void); void hook(void) { gone(); }' > overlay.c
          cc -shared -o overlay/gameoverlayrenderer.so overlay.c -Lgone -lgone
          preload=$PWD/overlay/gameoverlayrenderer.so
          if LD_PRELOAD=$preload ${pkgs.coreutils}/bin/true 2>/dev/null; then echo "the stand-in overlay failed nothing"; exit 1; fi
          LD_PRELOAD=$preload HOME=$PWD ${ui}/bin/universe-ui --help > /dev/null
          touch $out
        '';
      };

      nixosModules.default = import ./nix/nixos.nix {
        gsrPkg = pkgs.gpu-screen-recorder;
        uiPkg = ui;
      };
      homeModules.default = import ./nix/home-manager.nix {
        universePkg = universe;
        uiPkg = ui;
        desktopPkg = desktop;
        extensionPkg = universe-shell-extension;
      };
    };
}
