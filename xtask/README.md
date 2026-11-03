# ArtCraft Launcher workspace tasks

Ticket #70 delegates reusable tooling (`version`, `ico`, `icns`, and streaming `sha256_hex`) to
`craft-xtask` on `algo/integration`. Only this development crate depends on the library; runtime
packages are unchanged.

| Task | Shared implementation | Local compatibility policy |
|---|---|---|
| version | `version::run` | Default root manifest, staging in `target/`, offline refresh and online fallback |
| ico | `ico::run` | Caller-relative paths; square PNGs up to 256 px |
| icns | `ico::run_icns` | Caller-relative paths; square PNGs in 16/32/64/128/256/512/1024 px with 1x/2x density slots |
| local | `local::run` | Launcher-owned mirror sync/serve/status; uses `version::read` and `sha256_hex` |

`ci`, `wasm`, and `local` remain launcher-owned. Help text retains the launcher task table followed
by shared task syntax from `version::cli_help()` and `ico::cli_help()`.
