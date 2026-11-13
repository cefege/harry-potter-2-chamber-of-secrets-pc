# Native-free audit: LLVM support and evidence limits

The audit remains fail-closed. It now supports LLVM bitcode in selected Rust
library archives. This does **not** establish complete binary-crate coverage or
qualify a default change to LTO.

## Toolchain and first configure

The Rust pin remains `1.99.1`; `rust-toolchain.toml` requests `llvm-tools` alongside
clippy and rustfmt. CI and release toolchain installation must explicitly install
`llvm-tools`. The checker selects `llvm-nm` from the sysroot of Cargo's selected
compiler and checks its LLVM version. It does not fall back to Apple's older nm
when that tool cannot inspect current LLVM bitcode. Archive validation also uses
the matching `llvm-ar`.

CMake reads File API queries before it executes CMakeLists.txt. A query written
from a module during the first configure is too late. Desktop CI and release
workflows therefore seed the query first. For a new local build tree:

```sh
mkdir -p out/macos-arm64/.cmake/api/v1/query
touch out/macos-arm64/.cmake/api/v1/query/codemodel-v2
cmake --preset macos-arm64
cmake --build --preset macos-arm64 --target hp2_verification_binaries
for name in native_free_producer_contracts native_free_import native_free_ucc native_free_game; do
  ctest --preset macos-arm64 --no-tests=error --output-on-failure -R "^${name}$" || exit "$?"
done
```

Use the matching Linux preset and build directory on Linux. A macOS result does
not qualify Linux. The new query-preparation regression runs real fresh CMake
fixtures for all desktop workflow configure blocks; it is not a Linux product test.

## Provenance replay

The Cargo capture proxy retains its selected command, environment subset, Cargo
JSON events, artifact hashes, and build-script output hashes. Schema **3** records
absence semantics for all controlled variables, including `HOME`,
`RUSTUP_TOOLCHAIN`, compiler selectors, profile overrides, and incremental state.
Replay clears audit-time controlled variables before applying the record. Compiler
discovery also forwards the selected invocation's Cargo `--config` arguments.

Earlier capture schemas are rejected, not silently upgraded. After changing the
capture schema, preserve the old records and rerun the actual producer with the
same intended flags before auditing. Copying a version number into an old JSON
record does not establish the missing evidence.

`llvm-nm` can report success while skipping an unrecognized archive member. The
checker therefore compares all `llvm-ar` member names and multiplicities against
nm's output headers. An exact member-specific `no symbols` notice is acceptable
only with matching member/header evidence. Arbitrary diagnostics, malformed
members, missing artifacts, and missing provenance remain failures. A general
stderr suppression or `--quiet` alone is insufficient.

## Evidence and unresolved limits

The macOS arm64 producer-contract suite passes 41 tests, including malformed
bitcode, invalid embedded members, declaration-only members, tool diagnostics,
compiler precedence, Cargo configuration replay, and present/absent `HOME` cases.
Evidence is retained under `out/task-20261111/Artifacts/lto-audits/`.

A private fat-LTO build succeeded with 1 codegen unit and incremental compilation
off. Earlier product-audit failures and subsequent schema-1 passes are preserved.
Those records predate the final selector fixes. Schema-2 recapture rebuilt selected
artifacts; importer stayed byte-identical, while UCC and game changed. Changes in
fire/engine/wgpu inputs support an explanation, but the initial build did not hash
every source file at compilation. These builds are not established as equivalent
performance inputs.

Final schema-3 recapture and the 3 exact product audits passed once each. Every
record contains HOME, fat LTO, 1 codegen unit, and incremental compilation off.
All selected products rebuilt (`fresh=false`); executable hashes and complete
artifact-hash maps match schema 2. The 1,033 first-party source/manifest hashes
match before and after the build. These results qualify those private archive
checks, not the coverage limits or runtime/performance acceptance below.

Two coverage limits remain:

1. The current Rust symbol scan selects first-party `.rlib` files, not a separately
   captured binary-crate `main.rs` object. A forbidden ABI reference only in the
   binary crate can escape that scan. A future fix must bind the actual compiler
   invocation and emitted object to the linked executable and retained provenance.
2. Compiler selection does not yet bind the original compiler binary contents
   against later toolchain replacement. Recording an environment selector or
   checking the current LLVM version is not that identity proof.

A real fat-LTO probe confirmed that `--emit=obj,link,dep-info` retains the normal
executable and dep-info and emits an inspectable object. Cargo's JSON event did
not name that object. A cached rerun reported `fresh=true` but did not recreate a
removed object. A future sidecar implementation must reject missing/tampered
objects rather than treating a fresh Cargo result as proof of their presence.

Do not call the audit complete, enable LTO defaults, or claim a speedup from
compilation or archive checks alone. Matching behavioral, performance, data-profile,
and acceptance evidence is still required.
