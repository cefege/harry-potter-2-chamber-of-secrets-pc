# Proposed isolated overlay runtime test (not yet run)

This procedure is for a person at an interactive macOS arm64 desktop. It does not install or rebuild the shipping bundle. Use a disposable verified copy of the retail overlay; do not point the executable at the user's real saves or mutable retail data. Confirm the imported overlay against its manifest/checksums before the test, and record how it was verified. Keep its checksum results with the test evidence.

From the repository root, choose a new empty directory outside the checkout, for example `/tmp/hp2-overlay-manual`. The commands below create files there and build only a separate executable:

```sh
ROOT="$PWD"
TEST_ROOT=/tmp/hp2-overlay-manual
mkdir -p "$TEST_ROOT/home" "$TEST_ROOT/tmp" "$TEST_ROOT/data"
# Copy a verified retail overlay into $TEST_ROOT/data before proceeding.
cmake --preset macos-arm64 -B "$TEST_ROOT/build" \
  -DCMAKE_INSTALL_PREFIX="$TEST_ROOT/install" \
  -DHP2_EXPERIMENTAL_IN_GAME_OVERLAY=ON
cmake --build "$TEST_ROOT/build" --target hp2_game_rust
```

Check the build output and feature selection; do not install it. Launch the separate `$TEST_ROOT/build/hp2_game_rust` from a directory inside the disposable tree, with `HOME`, `CFFIXED_USER_HOME`, `TMPDIR`, and the XDG directories set inside it. Pass `-datadir=$TEST_ROOT/data` and a valid retail map such as `Entry.unr`. Do not pass replay, scripted input, or deterministic verification flags: those paths suppress physical input or overlay activation. Example, after populating `data`:

```sh
mkdir -p "$TEST_ROOT/home" "$TEST_ROOT/tmp" "$TEST_ROOT/xdg-config" "$TEST_ROOT/xdg-cache" "$TEST_ROOT/xdg-data"
cd "$TEST_ROOT"
HOME="$TEST_ROOT/home" CFFIXED_USER_HOME="$TEST_ROOT/home" TMPDIR="$TEST_ROOT/tmp" \
XDG_CONFIG_HOME="$TEST_ROOT/xdg-config" XDG_CACHE_HOME="$TEST_ROOT/xdg-cache" \
XDG_DATA_HOME="$TEST_ROOT/xdg-data" \
"$TEST_ROOT/build/hp2_game_rust" Entry.unr "-datadir=$TEST_ROOT/data"
```

Before interacting with settings, verify from the runtime log and actual files that its writable user root and active INI are inside `TEST_ROOT`; stop if not. The default runtime system INI is `HP.ini` in the selected user root, unless overridden. Verify no real save is available in the disposable home. Record the command, build feature evidence, overlay profile verification, log and screenshots in the test artifact directory.

Manually press F10 and confirm the menu is visible; exercise Display, Performance, category back, Escape, and F10 close. Confirm FPS changes immediately. Change VSync and frame limit, observe the restart-required message, and verify the active system INI. Restart the isolated process and verify the stored values reload; separately verify whether they actually affect rendering and frame timing. Test a forced config conflict, held movement release, focus loss, and window resize. Compare the retail data copy's checksums after the test. Record failures rather than retrying to obtain a pass. Repeat separately with prototype data; one profile does not establish behavior for another.

The exact build and launch commands above are a proposed path, not verified runtime evidence. A successful build alone does not establish that retail users can open or see the overlay.
