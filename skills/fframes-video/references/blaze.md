# Experimental Blaze builds

Use this workflow when the user asks to evaluate [Blaze](https://github.com/indexable-inc/blaze).
It needs no fframes dependency or feature change. Keep the project's release profile from
the main skill; the optional change is the build command.

## Current result

The CPU starter was checked on macOS arm64 on 2026-10-27:

| Blaze build | `run --release -- frame 2s` result |
| --- | --- |
| Published `nightly` archive, commit `8b15e4b` (reports `ra-jit 0.0.0`) | Rendered through ordinary Cargo; no accelerated release build. |
| CI build of `6ee705f`, LLVM level 3, Rust nightly-2026-10-27 | Failed before rendering: the `av-scenechange v0.14.1` build script emits `CARGO_CFG_TARGET_FEATURE` and `CARGO_ENCODED_RUSTFLAGS`, which Blaze rejects. |
| Locally built `6ee705f` with [six compatibility fixes](https://github.com/indexable-inc/blaze/pull/728), LLVM level 3 | Builds and renders with the original consumer dependencies and default sandboxing. |

The patched build passes 16 targeted checks. Five edited 1080p frames and a 24-frame contact
sheet have identical decoded pixels under Blaze and Cargo. It is still slower:

| Median of five warm runs | Cargo 1.99.1 | Patched Blaze |
| --- | ---: | ---: |
| Text edit through PNG creation | 1.44 s | 3.03 s |
| Existing binary rendering 24 full-size CPU frames in a contact sheet | 0.51 s | 14.74 s |

The comparison used separate projects and caches, alternating edit order, and the same
incremental release profile with 16 codegen units. The first build was excluded. The local
Blaze compiler used the official `dist` build with LLVM and no PGO; these results do not
describe every Blaze build. Keep Cargo as the default. Check the installed version and current
upstream behavior before retrying; the published archive, source and documentation can differ.
The CI artifact above also needed the system `libiconv` at runtime instead of its build
machine's Nix path. Prefer an upstream packaged build over distributing that CI artifact.

## Try a compatible build

Follow [Blaze's installation guide](https://github.com/indexable-inc/blaze/blob/main/docs/install.md)
if it is not installed. Use explicit `cargo blaze` commands in this project. `cargo blaze enable`
changes Cargo behavior for every project and is unnecessary here.

Blaze uses its own toolchain. Its code-generation settings are separate from Cargo profiles:
`--release` and `[profile.dev.package."*"] opt-level = 3` alone do not establish optimized
CPU rendering. Use a Blaze build with the LLVM backend and request level 3. This still does
not guarantee rustc's release performance, so measure rendering as well as rebuilding.

From the consumer project's root (the workspace root for a member project):

```sh
cargo blaze --version
cargo run --release -- frame 2s -o frames/cargo

B() {
  BLAZE_HOME="$PWD/target/blaze-home" CARGO_TARGET_DIR="$PWD/target/blaze" \
    BLAZE_BACKEND=llvm BLAZE_OPT=3 \
    cargo blaze run --release "$@"
}
B -- frame 2s -o frames/blaze
```

`B` accepts Cargo arguments before `--`, and video arguments after it. In a workspace, use
`B -p my-video -- frame 2s -o frames/blaze`; add `--bin` if needed. Keep its target directory
separate from ordinary Cargo output, and keep the same backend, features and output size in
both runs. The first build loads the compiler and dependencies; exclude it from warm timings.
The separate Blaze home also isolates its daemon and toolchain cache. The daemon captures
settings when it starts; stop it before changing the backend or optimization level:
`BLAZE_HOME="$PWD/target/blaze-home" cargo blaze stop`.

If a build fails, read the error and any worker log it names, report the failed dependency,
and resume the ordinary Cargo command. Do not remove media formats or disable build-script
isolation just to obtain a timing. A Cargo fallback is a working render, not a Blaze speedup.

## Decide whether to keep using it

After both commands render successfully, make the same small text edit for each tool and time
the complete command through PNG creation. Use several warm edits and compare their medians.
Open the PNGs and compare dimensions and pixels for the same source. Also time a representative
contact sheet or short CPU render through each built binary: a faster compiler can produce
slower rendering code.

Check `BLAZE_HOME="$PWD/target/blaze-home" cargo blaze status` and the build output to confirm the accelerated engine served the
edit. Only use Blaze for subsequent edits if the complete loop improves and the rendered result
is correct. Rebuild with Cargo after source or embedded-media changes before using its binary;
the two target directories contain separate builds. Use the project's ordinary Cargo toolchain
for final checks and `cargo run --release -- render` for the final export.

Keep Blaze's default proc-macro cache policy. `include_media_dir!` reads external files;
caching every function-like macro expansion can reuse stale embedded media. This workflow
restarts the command after an edit; it does not add live code replacement to the preview window.
