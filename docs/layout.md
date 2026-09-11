# Dépôt — disposition

```
Cargo.toml, crates/universe/     cœur Rust : lib + bins `universe` (CLI) et `universed` (démon)
ui/                              hôte PySide6 : paquet `universe_ui/` (python + qml/), tests/, pyproject.toml
modules/<id>/                    modules livrés : module.toml + bin/* (+ tests/)
nix/                             module home-manager, module NixOS
flake.nix
docs/                            plan-v3.html, api.md (contrat), progress.md (source de vérité), layout.md
```

Règles de collision entre agents : un agent n'écrit que dans son dossier (ui/, modules/<id>/, crates/) et éventuellement docs/<son-sujet>.md. `docs/progress.md` n'est écrit que par l'agent principal.

Tests : `cargo test` (cœur), `pytest ui` (config dans `ui/pyproject.toml`, `QT_QPA_PLATFORM=offscreen`) et `pytest modules` (chaque module porte ses `tests/`). Deux invocations distinctes : les deux arbres n'ont ni le même runtime ni la même config.

Python : PySide6 6.11 via `nix shell --impure --expr 'let pkgs = import <nixpkgs> {}; in pkgs.python3.withPackages (ps: [ ps.pyside6 ps.pytest ps.pysdl2 ])' -c …` (ajouter ce qu'il faut). Chemins d'import QML à calculer (docs/probes/qml_import_paths.py) tant que le flake ne wrappe pas l'hôte.
