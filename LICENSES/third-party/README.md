# Licence texts the notices quote

`scripts/third-party-notices.py` copies these into `THIRD-PARTY-NOTICES.md` and
`THIRD-PARTY-NOTICES-GPL.md`. They are here because their packages do not carry them: the
Iconify icon packages ship only the name of each licence, libpng is not part of this
repository, and the notices must be checkable in CI without installing anything. Each is
the upstream text, unmodified, retrieved on 2026-10-01:

| File | Covers | Source |
|---|---|---|
| `bootstrap-icons.txt` | Bootstrap Icons (`@iconify-json/bi`) | <https://raw.githubusercontent.com/twbs/icons/main/LICENSE> |
| `iconoir.txt` | Iconoir (`@iconify-json/iconoir`) | <https://raw.githubusercontent.com/iconoir-icons/iconoir/main/LICENSE> |
| `lucide.txt` | Lucide (`@iconify-json/lucide`) | <https://raw.githubusercontent.com/lucide-icons/lucide/main/LICENSE> |
| `vscode-icons.txt` | VSCode Icons (`@iconify-json/vscode-icons`) | <https://raw.githubusercontent.com/vscode-icons/vscode-icons/master/LICENSE> |
| `apache-2.0.txt` | Google Material Icons, Material Symbols, Material Design Icons (`@iconify-json/ic`, `material-symbols`, `mdi`) | <https://raw.githubusercontent.com/material-icons/material-icons/master/LICENSE>, identical to google/material-design-icons' |
| `pictogrammers-free-license.txt` | Material Design Icons (`@iconify-json/mdi`) | <https://raw.githubusercontent.com/Templarian/MaterialDesign/master/LICENSE> |
| `fira-code.txt` | Fira Code, the window's font (`@fontsource-variable/fira-code`), and the OFL text for Elusive Icons | the package's own `LICENSE` |
| `libpng.txt` | libpng 1.6.58, whose gamma arithmetic `crates/skidbladnir-encode/src/png_gamma.rs` reproduces | libpng 1.6.58's `LICENSE`, as Arch Linux's `libpng` package installs it |

Update a file when its package's licence changes; the generator checks that the icon
collections it describes are exactly those in `frontend/package.json`.
