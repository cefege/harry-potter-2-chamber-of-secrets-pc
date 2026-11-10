# Root of the immutable prototype data tree handed to behavioral tests via
# -datadir/--data-root. Defaults to the build's prototype checkout so test
# runs can be pointed at an alternate data root without editing this file.
set(HP2_TEST_DATA_ROOT "${HP2_UNREAL_ROOT}" CACHE PATH
    "Prototype data root passed to behavioral tests via -datadir/--data-root")
# A packaged Rust product must never become the default native reference.
# CACHE without FORCE preserves existing values for explicit owner review.
set(HP2_CPP_ORACLE_APP "" CACHE PATH
    "Candidate C++ oracle path; not acceptance, native parity is blocked")

# The Rust workspace builds the shipping products through corrosion: hp2_game_rust,
# hp2_ucc_rust, and the standalone hp2_import importer CLI. Each is imported and
# configured below; the former native (C++) engine target graph is gone.
corrosion_import_crate(MANIFEST_PATH "${PROJECT_SOURCE_DIR}/Cargo.toml"
        CRATES hp2-import)
    corrosion_import_crate(MANIFEST_PATH "${PROJECT_SOURCE_DIR}/Cargo.toml"
        CRATES hp2-ucc hp2-game NO_LINKER_OVERRIDE)
    set_property(TARGET hp2_ucc_rust PROPERTY HP2_NATIVE_FREE_CARGO_MANIFEST
        "${PROJECT_SOURCE_DIR}/crates/hp2-ucc/Cargo.toml")
    set_property(TARGET hp2_game_rust PROPERTY HP2_NATIVE_FREE_CARGO_MANIFEST
        "${PROJECT_SOURCE_DIR}/crates/hp2-game/Cargo.toml")
    set_property(TARGET hp2_import PROPERTY HP2_NATIVE_FREE_CARGO_MANIFEST
        "${PROJECT_SOURCE_DIR}/crates/hp2-import/Cargo.toml")
    hp2_enable_cargo_provenance(hp2_ucc_rust)
    hp2_enable_cargo_provenance(hp2_game_rust)
    hp2_enable_cargo_provenance(hp2_import)
    corrosion_set_features(hp2_import FEATURES archive)
    # Corrosion's binary linker does not traverse CMake INTERFACE libraries.
    # Link the pinned archive and compression targets directly.
    corrosion_link_libraries(hp2_import
        archive_static zlibstatic hp2_bzip2 liblzma)
    corrosion_link_libraries(hp2_game_rust
        SDL2::SDL2-static OpenAL::OpenAL archive_static
        zlibstatic hp2_bzip2 liblzma)
    if(APPLE)
        # Corrosion does not propagate SDL's framework dependencies either.
        # Use the configured SDK's flags, shared with the Cargo behavior tests.
        get_target_property(HP2_SDL_BINARY_DIR SDL2::SDL2-static BINARY_DIR)
        execute_process(
            COMMAND sh "${HP2_SDL_BINARY_DIR}/sdl2-config" --static-libs
            RESULT_VARIABLE HP2_SDL_LINK_STATUS
            OUTPUT_VARIABLE HP2_SDL_LINK_TEXT
            OUTPUT_STRIP_TRAILING_WHITESPACE
        )
        if(NOT HP2_SDL_LINK_STATUS EQUAL 0)
            message(FATAL_ERROR "Cannot resolve configured SDL static SDK dependencies")
        endif()
        separate_arguments(HP2_SDL_STATIC_LINK_ITEMS UNIX_COMMAND "${HP2_SDL_LINK_TEXT}")
        foreach(HP2_SDL_LINK_ITEM IN LISTS HP2_SDL_STATIC_LINK_ITEMS)
            if(HP2_SDL_LINK_ITEM MATCHES "^(-l|-Wl,)" AND NOT HP2_SDL_LINK_ITEM STREQUAL "-lSDL2")
                corrosion_add_target_local_rustflags(hp2_game_rust "-Clink-arg=${HP2_SDL_LINK_ITEM}")
            endif()
        endforeach()
        set(HP2_NFD_RUST_FRAMEWORK_FLAGS)
        foreach(HP2_NFD_FRAMEWORK IN ITEMS "${APPKIT_LIBRARY}" "${UNIFORMTYPEIDENTIFIERS_LIBRARY}")
            if(HP2_NFD_FRAMEWORK)
                get_filename_component(HP2_NFD_FRAMEWORK_NAME "${HP2_NFD_FRAMEWORK}" NAME_WE)
                list(APPEND HP2_NFD_RUST_FRAMEWORK_FLAGS "-lframework=${HP2_NFD_FRAMEWORK_NAME}")
            endif()
        endforeach()
        corrosion_add_target_local_rustflags(hp2_game_rust
            ${HP2_NFD_RUST_FRAMEWORK_FLAGS}
            "-lframework=QuartzCore" "-lframework=CoreGraphics" "-lframework=Metal"
            "-lframework=Foundation" "-lframework=CoreFoundation" "-lframework=AppKit"
            "-lframework=CoreText" "-lobjc" "-liconv"
            "-Clink-arg=-mmacosx-version-min=${CMAKE_OSX_DEPLOYMENT_TARGET}"
            "-Clink-arg=-Wl,-rpath,$<TARGET_FILE_DIR:OpenAL::OpenAL>")
    endif()

# Build every executable the behavioral test graph can invoke before CTest
# starts, so sanitizer/preset runs never register tests that point at missing
# binaries. Script-backed tests (Python) need no compiled target.
add_custom_target(hp2_verification_binaries
    DEPENDS hp2_import hp2_ucc_rust hp2_game_rust
    COMMENT "Building every executable referenced by the HP2 behavioral test graph"
)

# A test that needs game data is only registered when the configured data root
# actually provides it. Without this guard a fresh clone (no data, by design)
# registers tests that abort on a missing System/Default.ini, so a plain
# `ctest --preset macos-arm64` reports failures no checkout can fix. This is a
# configuration gap, not a skip: the tests exist wherever the data does. See
# Docs/BEHAVIOR_MATRIX.md for the data-profile contract.
set(HP2_TEST_DATA_PRESENT OFF)
if(EXISTS "${HP2_TEST_DATA_ROOT}/System/Default.ini")
    set(HP2_TEST_DATA_PRESENT ON)
endif()
message(STATUS "HP2 test data present at ${HP2_TEST_DATA_ROOT}: ${HP2_TEST_DATA_PRESENT}")

# Give every behavioral test an isolated, deterministic HOME. HP2 derives its
# writable Application Support tree from HOME; immutable package data is
# always selected explicitly with -datadir when runtime bootstrap is needed.
# Each test also owns a stable artifact directory exposed through
# HP2_ARTIFACT_DIR so structured reports, logs, and captures land in one
# predictable place instead of ad-hoc stdout.
function(hp2_add_behavior_test test_name target)
    set(_hp2_test_root "${CMAKE_BINARY_DIR}/Testing/HP2/${test_name}")
    set(_hp2_test_home "${_hp2_test_root}/Home")
    set(_hp2_test_tmp "${_hp2_test_root}/Tmp")
    set(_hp2_test_artifacts "${_hp2_test_root}/Artifacts")
    set(_hp2_test_xdg_config "${_hp2_test_home}/.config")
    set(_hp2_test_xdg_cache "${_hp2_test_home}/.cache")
    set(_hp2_test_xdg_data "${_hp2_test_home}/.local/share")
    file(MAKE_DIRECTORY "${_hp2_test_home}" "${_hp2_test_tmp}" "${_hp2_test_artifacts}"
        "${_hp2_test_xdg_config}" "${_hp2_test_xdg_cache}" "${_hp2_test_xdg_data}")

    if(UNIX)
        # Compiler-cache startup sockets must fit sockaddr_un even when the
        # checkout/artifact path is long. Keep the physical per-test Tmp tree;
        # expose it through a private, short alias rather than disable caching.
        if(NOT IS_DIRECTORY "${HP2_TEST_TMP_ALIAS_ROOT}")
            execute_process(COMMAND mktemp -d /tmp/hp2-tests.XXXXXXXX
                RESULT_VARIABLE _hp2_tmp_status
                OUTPUT_VARIABLE _hp2_tmp_root
                OUTPUT_STRIP_TRAILING_WHITESPACE)
            if(NOT _hp2_tmp_status STREQUAL "0")
                message(FATAL_ERROR "Cannot create private test temporary alias root")
            endif()
            set(HP2_TEST_TMP_ALIAS_ROOT "${_hp2_tmp_root}" CACHE INTERNAL
                "Private short aliases for per-test Unix temporary directories" FORCE)
        endif()
        string(SHA256 _hp2_tmp_id "${_hp2_test_tmp}")
        string(SUBSTRING "${_hp2_tmp_id}" 0 16 _hp2_tmp_id)
        set(_hp2_tmp_alias "${HP2_TEST_TMP_ALIAS_ROOT}/${_hp2_tmp_id}")
        if(NOT IS_SYMLINK "${_hp2_tmp_alias}")
            file(CREATE_LINK "${_hp2_test_tmp}" "${_hp2_tmp_alias}" SYMBOLIC
                RESULT _hp2_tmp_status)
            if(NOT _hp2_tmp_status STREQUAL "0")
                message(FATAL_ERROR "Cannot link test temporary directory: ${_hp2_tmp_status}")
            endif()
        endif()
        set(_hp2_test_tmp "${_hp2_tmp_alias}")
    endif()

    hp2_prepare_cargo_test_command(_hp2_test_command "${target}" ${ARGN})
    add_test(NAME "${test_name}" COMMAND ${_hp2_test_command})
    set(_hp2_test_env
        "HOME=${_hp2_test_home}"
        "TMPDIR=${_hp2_test_tmp}"
        # Isolate every XDG base directory too: a real desktop session's
        # $XDG_DATA_HOME would otherwise outrank the redirected $HOME when
        # HP2Paths resolves the writable Application Support tree on Linux,
        # leaking test state into (and reading stale state from) the host.
        "XDG_CONFIG_HOME=${_hp2_test_xdg_config}"
        "XDG_CACHE_HOME=${_hp2_test_xdg_cache}"
        "XDG_DATA_HOME=${_hp2_test_xdg_data}"
        "LC_ALL=C"
        "TZ=UTC"
        "HP2_TEST_NAME=${test_name}"
        "HP2_ARTIFACT_DIR=${_hp2_test_artifacts}"
    )
    if(HP2_ENABLE_ASAN_UBSAN)
        list(APPEND _hp2_test_env
            "HP2_SANITIZER=asan-ubsan"
            # Leak checks stay on; ASan reports land beside the other test
            # artifacts instead of stderr so structured reports stay parseable.
            "ASAN_OPTIONS=detect_leaks=1:log_path=${_hp2_test_artifacts}/asan"
        )
    elseif(HP2_ENABLE_TSAN)
        list(APPEND _hp2_test_env
            "HP2_SANITIZER=tsan"
            "TSAN_OPTIONS=halt_on_error=1"
        )
    endif()
    # Every test gets a data-profile label by default. CI selects with
    # `ctest -L data-none`, so a test with no LABELS property is silently
    # omitted from the run a data-less checkout can execute. Deriving the
    # label from the arguments - rather than a hand-maintained name list -
    # keeps a newly registered data-backed test from being dropped because
    # someone forgot a set_tests_properties line elsewhere in this file.
    set(_hp2_default_labels "fast;data-none")
    if(HP2_TEST_DATA_PRESENT)
        foreach(_hp2_arg IN LISTS ARGN)
            if(_hp2_arg MATCHES "HP2_TEST_DATA_ROOT"
               OR _hp2_arg MATCHES "^--?datadir="
               OR _hp2_arg MATCHES "^--?data-root=")
                set(_hp2_default_labels "integration;data-prototype")
                break()
            endif()
        endforeach()
    endif()
    set_tests_properties("${test_name}" PROPERTIES
        WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}"
        # Deterministic contracts must never hang a verification run; a
        # timeout is a first-class failure with retained artifacts.
        TIMEOUT 900
        ENVIRONMENT "${_hp2_test_env}"
        LABELS "${_hp2_default_labels}"
    )
endfunction()

find_package(Python3 REQUIRED COMPONENTS Interpreter)
    hp2_add_behavior_test(importer_parity "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/ImporterParityTests.py"
        "$<TARGET_FILE:hp2_import>"
        "${PROJECT_SOURCE_DIR}/Build/prepare_retail_data.py"
    )
    set_tests_properties(importer_parity PROPERTIES LABELS "fast;data-none")
    hp2_add_behavior_test(import_cli_contracts "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/ImportCliTests.py"
        "--importer" "$<TARGET_FILE:hp2_import>"
    )
    set_tests_properties(import_cli_contracts PROPERTIES LABELS "fast;data-none")
    hp2_add_behavior_test(import_path_process_contracts "${Python3_EXECUTABLE}" -B
        "${PROJECT_SOURCE_DIR}/Tests/ImportPathSafetyTests.py"
        "--importer" "$<TARGET_FILE:hp2_import>"
    )
    set_tests_properties(import_path_process_contracts PROPERTIES LABELS "fast;data-none")

# Metadata-only probes enforce the pointer contract without linking SDL or
# executing an invalid window pointer. Use the configured, pinned compiler.
hp2_add_behavior_test(platform_window_pointer_contracts "${Python3_EXECUTABLE}" -B
    "${PROJECT_SOURCE_DIR}/Tests/WindowPointerSafetyTests.py"
    "--source" "${PROJECT_SOURCE_DIR}/crates/hp2-platform/src/window_lifecycle.rs"
    "--rustc" "$<TARGET_FILE:Rust::Rustc>"
)
set_tests_properties(platform_window_pointer_contracts PROPERTIES LABELS "fast;data-none")

# Fake NFD/SDL calls exercise actual picker source in isolated processes. The
# pthread predicate is real; these checks do not open or qualify native dialogs.
if(CMAKE_SYSTEM_NAME STREQUAL "Darwin")
    hp2_add_behavior_test(platform_picker_admission_contracts "${Python3_EXECUTABLE}" -B
        "${PROJECT_SOURCE_DIR}/Tests/PickerSafetyTests.py"
        "--source" "${PROJECT_SOURCE_DIR}/crates/hp2-platform/src/picker.rs"
        "--rustc" "$<TARGET_FILE:Rust::Rustc>"
        "--clang" "${CMAKE_C_COMPILER}"
    )
    set_tests_properties(platform_picker_admission_contracts PROPERTIES LABELS "fast;data-none")
endif()

# Deterministic build/observation fingerprint: writes out/<preset>/
# hp2-state.json at configure time (toolchain, capability gates, data
# availability) and offers a refreshable drift check target.
execute_process(
    COMMAND "${Python3_EXECUTABLE}" "${PROJECT_SOURCE_DIR}/Build/hp2_state.py"
            "--build-dir" "${CMAKE_BINARY_DIR}"
    RESULT_VARIABLE _hp2_state_result
    ERROR_VARIABLE _hp2_state_err
)
if(NOT _hp2_state_result EQUAL 0)
    message(WARNING "hp2_state failed (${_hp2_state_result}): ${_hp2_state_err}")
endif()

add_custom_target(hp2_state
    COMMAND "${Python3_EXECUTABLE}" "${PROJECT_SOURCE_DIR}/Build/hp2_state.py"
            "--build-dir" "${CMAKE_BINARY_DIR}" --check
    WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}"
    COMMENT "Validating hp2-state.json drift")

find_program(HP2_CARGO_EXECUTABLE cargo REQUIRED)
if(DEFINED ENV{CARGO_HOME} AND NOT "$ENV{CARGO_HOME}" STREQUAL "")
    set(HP2_CARGO_HOME "$ENV{CARGO_HOME}")
else()
    set(HP2_CARGO_HOME "$ENV{HOME}/.cargo")
endif()

hp2_add_behavior_test(import_path_safety_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/import_path_safety_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-import
    --lib
    path_safety_
    --
    --test-threads=1
)
set_tests_properties(import_path_safety_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(import_library_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/import_library_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-import
    --lib
    --
    --test-threads=1
)
set_tests_properties(import_library_contracts PROPERTIES LABELS "fast;data-none")

hp2_add_behavior_test(core_config_safety "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_config_safety/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=config_safety_contracts
    --
    --test-threads=1
)

hp2_add_behavior_test(core_owner_paths_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_owner_paths_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=runtime_owner_paths_success
    --test=runtime_owner_paths_five_paths
    --
    --test-threads=1
)
set_tests_properties(core_owner_paths_contracts PROPERTIES LABELS "fast;data-none")

hp2_add_behavior_test(engine_settings_launcher_parity "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/engine_settings_launcher_parity/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=settings_launcher_parity
    --
    --test-threads=1
)

hp2_add_behavior_test(engine_settings_config "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/engine_settings_config/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=settings_config_contracts
    --
    --test-threads=1
)

# These focused tests share the existing settings Cargo cache to avoid another
# full workspace dependency tree. Cargo serializes concurrent writers itself.
set(HP2_STOCK_FPS_TEST_CARGO_DIR "${CMAKE_BINARY_DIR}/Testing/HP2/engine_settings_config/Cargo")
hp2_add_behavior_test(stock_fps_adapter_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --lib stock_fps_menu::tests -- --test-threads=1
)
set_tests_properties(stock_fps_adapter_contracts PROPERTIES LABELS "fast;data-none")

hp2_add_behavior_test(stock_graphics_menu_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --lib stock_graphics_menu::tests -- --test-threads=1
)
set_tests_properties(stock_graphics_menu_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(stock_graphics_settings "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --test stock_graphics_settings -- --test-threads=1
)
set_tests_properties(stock_graphics_settings PROPERTIES LABELS "fast;data-none")

hp2_add_behavior_test(stock_graphics_gpu_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}" "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --test stock_graphics_gpu -- --test-threads=1)
set_tests_properties(stock_graphics_gpu_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(stock_graphics_gpu_backend_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}" "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --lib stock_graphics_gpu_backend::tests -- --test-threads=1)
set_tests_properties(stock_graphics_gpu_backend_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(live_texture_detail_policy_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}" "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --lib live_texture_detail::policy::tests -- --test-threads=1)
set_tests_properties(live_texture_detail_policy_contracts PROPERTIES LABELS "fast;data-none")
if(HP2_TEST_DATA_PRESENT)
    hp2_add_behavior_test(stock_resolution_lifecycle_vm "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}" "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
        "HP2_CORE_DATA_ROOT=${HP2_TEST_DATA_ROOT}"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-engine --lib stock_resolution_lifecycle::tests -- --test-threads=1)
    set_tests_properties(stock_resolution_lifecycle_vm PROPERTIES LABELS "fast;data-prototype")
    hp2_add_behavior_test(stock_render_scale_slider_vm "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}" "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
        "HP2_CORE_DATA_ROOT=${HP2_TEST_DATA_ROOT}"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-engine --features=renderer-wgpu --lib stock_render_scale_control::tests::loaded_slider -- --ignored --test-threads=1)
    set_tests_properties(stock_render_scale_slider_vm PROPERTIES LABELS "fast;data-prototype")
    hp2_add_behavior_test(stock_input_owner_contracts "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}" "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
        "HP2_CORE_DATA_ROOT=${HP2_TEST_DATA_ROOT}"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-engine --lib input_owner:: -- --test-threads=1)
    set_tests_properties(stock_input_owner_contracts PROPERTIES LABELS "fast;data-prototype")
endif()

foreach(_hp2_python_contract IN ITEMS
    RetailStockSettingsContracts RetailStockAdvancedGraphicsContracts RetailStockGraphicsPopupContracts
    RetailDisplayModeContracts RetailAdvancedGraphicsFixtureTests RetailModernResolutionFixtureTests)
    hp2_add_behavior_test("${_hp2_python_contract}" "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/${_hp2_python_contract}.py")
    set_tests_properties("${_hp2_python_contract}" PROPERTIES LABELS "fast;data-none")
endforeach()
if(HP2_TEST_DATA_PRESENT)
    hp2_add_behavior_test(StockInputBytecodeTests "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/StockInputBytecodeTests.py")
    set_tests_properties(StockInputBytecodeTests PROPERTIES LABELS "fast;data-prototype")
endif()

# Render-scale checks use Cargo test binaries only; they never rebuild the game.
hp2_add_behavior_test(stock_render_scale_policy "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --test=settings_policy_contracts render_scale_accepts_slider_steps_and_legacy_choice -- --test-threads=1
)
hp2_add_behavior_test(stock_render_scale_control "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --features=renderer-wgpu --lib stock_render_scale_ -- --test-threads=1
)
# These include ignored tests that create a real GPU device. Serialize them
# with the other GPU behavior tests and reuse the shared Cargo test cache.
hp2_add_behavior_test(live_graphics_wrapper_gpu "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --features=renderer-wgpu --lib
    render_device_owner::live_graphics_wrapper_tests -- --include-ignored --test-threads=1
)
set_tests_properties(live_graphics_wrapper_gpu PROPERTIES
    LABELS "integration;data-none;renderer;wgpu" RESOURCE_LOCK hp2_gpu TIMEOUT 600)
hp2_add_behavior_test(stock_render_scale_launcher_parity "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --test=settings_launcher_parity -- --test-threads=1
)
hp2_add_behavior_test(perf_harness_contracts "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/PerfHarnessTests.py")
set_tests_properties(perf_harness_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(stock_render_scale_replay_contracts "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/RetailStockRenderScaleContracts.py")
set_tests_properties(stock_render_scale_policy stock_render_scale_control stock_render_scale_launcher_parity stock_render_scale_replay_contracts
    PROPERTIES LABELS "fast;data-none")

# Focused stock resolution contracts. These do not launch the game or require
# SDL display hardware; the opt-in retail replay below covers the live menu.
hp2_add_behavior_test(stock_resolution_popup_callbacks "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --lib stock_menu_extensions::tests -- --test-threads=1
)
set_tests_properties(stock_resolution_popup_callbacks PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(stock_resolution_menu_policy "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --lib stock_resolution_menu::tests -- --test-threads=1
)
set_tests_properties(stock_resolution_menu_policy PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(stock_resolution_discovery_policy "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --lib camera_owner::sdl_display_exec::tests -- --test-threads=1
)
set_tests_properties(stock_resolution_discovery_policy PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(stock_resolution_replay_contracts "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/RetailModernResolutionContracts.py")
set_tests_properties(stock_resolution_replay_contracts PROPERTIES LABELS "fast;data-none")

option(HP2_ENABLE_RETAIL_RESOLUTION_VISUAL "Enable the windowed retail resolution replay and visual-review captures" OFF)
if(HP2_ENABLE_RETAIL_RESOLUTION_VISUAL)
    if(NOT EXISTS "${HP2_TEST_DATA_ROOT}/overlay-manifest.json")
        message(FATAL_ERROR "HP2_ENABLE_RETAIL_RESOLUTION_VISUAL requires a retail overlay manifest")
    endif()
    hp2_add_behavior_test(stock_resolution_retail_visual "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/RetailModernResolutionSmoke.py"
        "--data-root=${HP2_TEST_DATA_ROOT}"
        "--engine-bin=${CMAKE_BINARY_DIR}/hp2_game_rust"
        --allow-window)
    set_tests_properties(stock_resolution_retail_visual PROPERTIES LABELS "data-retail;renderer;visual-review" RUN_SERIAL TRUE TIMEOUT 300)
endif()

# The real VM hook fixture copies these prototype packages into the isolated
# artifact root; it does not require or claim verified retail data.
if(EXISTS "${PROJECT_SOURCE_DIR}/HarryPotter2/Unreal/System/Core.u"
   AND EXISTS "${PROJECT_SOURCE_DIR}/HarryPotter2/Unreal/System/Default.ini"
   AND EXISTS "${PROJECT_SOURCE_DIR}/HarryPotter2/Unreal/System/DefUser.ini")
    hp2_add_behavior_test(core_script_post_call_hook "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}"
        "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline
        "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-core --test=script_post_call_hook -- --test-threads=1
    )
    set_tests_properties(core_script_post_call_hook PROPERTIES LABELS "fast;data-prototype")
endif()

hp2_add_behavior_test(stock_fps_replay_contracts "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/RetailStockFpsContracts.py"
)
set_tests_properties(stock_fps_replay_contracts PROPERTIES LABELS "fast;data-none")

hp2_add_behavior_test(wgpu_frame_checkpoint_filter "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${HP2_STOCK_FPS_TEST_CARGO_DIR}"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-wgpu --lib checkpoint_filter -- --test-threads=1
)
set_tests_properties(wgpu_frame_checkpoint_filter PROPERTIES LABELS "fast;data-none")

hp2_add_behavior_test(engine_settings_policy "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/engine_settings_policy/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=settings_policy_contracts
    --
    --test-threads=1
)

hp2_add_behavior_test(input_binding_foundation "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/input_binding_foundation/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=binding_foundation
    --test=binding_capture_release
    --test=binding_parser_parity
)
set_tests_properties(input_binding_foundation PROPERTIES LABELS "unit;data-none")

hp2_add_behavior_test(display_fullscreen_mouse "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/display_fullscreen_mouse/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-platform --lib fullscreen_mouse_mapping
)
set_tests_properties(display_fullscreen_mouse PROPERTIES LABELS "unit;data-none")

hp2_add_behavior_test(input_binding_analysis "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/input_binding_foundation/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --test=binding_analysis
)
set_tests_properties(input_binding_analysis PROPERTIES LABELS "unit;data-none")

hp2_add_behavior_test(input_binding_alias_guard "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/input_binding_foundation/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --lib binding_analysis::tests
)
set_tests_properties(input_binding_alias_guard PROPERTIES LABELS "unit;data-none")

hp2_add_behavior_test(input_binding_runtime "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/input_binding_foundation/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --lib input_binding_runtime::tests
    -- --test-threads=1
)
set_tests_properties(input_binding_runtime PROPERTIES LABELS "integration;data-prototype")

hp2_add_behavior_test(input_binding_rebind "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/input_binding_foundation/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --lib input_binding_runtime::rebind_tests
    -- --test-threads=1
)
set_tests_properties(input_binding_rebind PROPERTIES LABELS "integration;data-prototype")

hp2_add_behavior_test(input_binding_live_analysis "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/input_binding_foundation/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine --lib binding_live_analysis::tests
    -- --test-threads=1
)
set_tests_properties(input_binding_live_analysis PROPERTIES LABELS "integration;data-prototype")

hp2_add_behavior_test(input_binding_config "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/input_binding_foundation/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=binding_config
)
set_tests_properties(input_binding_config PROPERTIES LABELS "unit;data-none")

hp2_add_behavior_test(core_parse_integer "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_parse_integer/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    parse_scalar
    --
    --test-threads=1
)
hp2_add_behavior_test(core_parse_scalar "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_parse_scalar/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    parse_scalar
    --
    --test-threads=1
)
hp2_add_behavior_test(core_parse_guid "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_parse_guid/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    guid
    --
    --test-threads=1
)
hp2_add_behavior_test(core_guid_generation "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_guid_generation/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    guid
    --
    --test-threads=1
)
hp2_add_behavior_test(core_native_path "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_native_path/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    path
    --
    --test-threads=1
)
hp2_add_behavior_test(core_parse_object "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_parse_object/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    parse_object
    --
    --test-threads=1
)
hp2_add_behavior_test(core_pure_ansi "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_pure_ansi/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    ansi
    --
    --test-threads=1
)
hp2_add_behavior_test(core_trim_spaces "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_trim_spaces/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    ansi
    --
    --test-threads=1
)
hp2_add_behavior_test(core_memswap "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_memswap/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    mem
    --
    --test-threads=1
)
hp2_add_behavior_test(core_crc "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_crc/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    crc
    --
    --test-threads=1
)
hp2_add_behavior_test(core_spc "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_spc/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    spc
    --
    --test-threads=1
)
hp2_add_behavior_test(core_md5 "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_md5/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    md5
    --
    --test-threads=1
)
hp2_add_behavior_test(core_ceil_log_two "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_ceil_log_two/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    math
    --
    --test-threads=1
)
hp2_add_behavior_test(core_app_fext "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_app_fext/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    app_format
    --
    --test-threads=1
)
hp2_add_behavior_test(core_app_path_leaf "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_app_path_leaf/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    app_format
    --
    --test-threads=1
)
hp2_add_behavior_test(command_line_load "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/command_line_load/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    command
    --
    --test-threads=1
)
hp2_add_behavior_test(compact_index "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/compact_index/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    compact_index
    --
    --test-threads=1
)
hp2_add_behavior_test(fstring_archive "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/fstring_archive/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    fstring_archive
    --
    --test-threads=1
)
hp2_add_behavior_test(ftime_archive "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/ftime_archive/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    ftime_archive
    --
    --test-threads=1
)
hp2_add_behavior_test(rng_seed_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/rng_seed_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    rng
    --
    --test-threads=1
)

hp2_add_behavior_test(engine_actor_anim "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/engine_actor_anim/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    actor_anim::
)
set_tests_properties(engine_actor_anim PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_inline_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_inline_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    actor_inline_tests::
)
set_tests_properties(actor_inline_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(engine_world_collision_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/engine_world_collision_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    --config=profile.test.opt-level=1
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=actcol_fixtures
    --test=dynbsp_fixtures
    --test=mover_fixtures
    --test=mesh_fixtures
    --test=skeletal_scene
    --test=level_core_replay
    --test=uengine_replay
    --test=actor_unactor
    --
    --test-threads=1
)
set_tests_properties(engine_world_collision_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(audio_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/audio_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    audio_tests::
)
set_tests_properties(audio_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_audio_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_audio_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=actor_audio_contracts
)
set_tests_properties(actor_audio_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_spawn_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_spawn_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=actor_spawn_contracts
)
set_tests_properties(actor_spawn_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_save_game_exists "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_save_game_exists/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    actor::save_game_exists_tests
)
set_tests_properties(actor_save_game_exists PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_iterator_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_iterator_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=actor_iterator_contracts
)
set_tests_properties(actor_iterator_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_iterator_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_iterator_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=actor_iterator_owner_contracts
)
set_tests_properties(actor_iterator_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(pawn_native_ai_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/pawn_native_ai_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=pawn_native_ai_owner_contracts
)
set_tests_properties(pawn_native_ai_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(pawn_reach_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/pawn_reach_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=pawn_reach_owner_contracts
)
set_tests_properties(pawn_reach_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_audio_vm_native_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_audio_vm_native_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=actor_audio_vm_native_owner_contracts
    --
    --test-threads=1
)
set_tests_properties(actor_audio_vm_native_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(pawn_path_vm_native_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/pawn_path_vm_native_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=pawn_path_vm_native_owner_contracts
    --
    --test-threads=1
)
set_tests_properties(pawn_path_vm_native_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(pawn_native_move_fixtures "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/pawn_native_move_fixtures/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=pawn_native_move_fixtures
    --
    --test-threads=1
)
set_tests_properties(pawn_native_move_fixtures PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(pawn_move_vm_native_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/pawn_move_vm_native_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=pawn_move_vm_native_owner_contracts
    --
    --test-threads=1
)
set_tests_properties(pawn_move_vm_native_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_script_vm_native_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_script_vm_native_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=actor_script_vm_native_owner_contracts
    --
    --test-threads=1
)
set_tests_properties(actor_script_vm_native_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(console_vm_native_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/console_vm_native_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=console_vm_native_owner_contracts
    --
    --test-threads=1
)
set_tests_properties(console_vm_native_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(scripted_texture_vm_native_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/scripted_texture_vm_native_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=scripted_texture_vm_native_owner_contracts
    --
    --test-threads=1
)
set_tests_properties(scripted_texture_vm_native_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_file_vm_native_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_file_vm_native_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=actor_file_vm_native_owner_contracts
    --
    --test-threads=1
)
set_tests_properties(actor_file_vm_native_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_file_transfer_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_file_transfer_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=actor_file_transfer_owner_contracts
    --
    --test-threads=1
)
set_tests_properties(actor_file_transfer_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(decal_vm_native_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/decal_vm_native_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=decal_vm_native_owner_contracts
    --
    --test-threads=1
)
set_tests_properties(decal_vm_native_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(stat_log_vm_native_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/stat_log_vm_native_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=stat_log_vm_native_owner_contracts
    --
    --test-threads=1
)
set_tests_properties(stat_log_vm_native_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(editor_transaction_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/editor_transaction_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=editor_transaction_owner_contracts
    --
    --test-threads=1
)
set_tests_properties(editor_transaction_owner_contracts PROPERTIES LABELS "fast;data-none")
if(APPLE)
    # Cargo's final executable, like hp2_game, links the one CMake-owned SDL
    # archive. Read its generated dependency flags rather than pinning a second
    # list of Apple frameworks in the Rust crate.
    set(HP2_B5_GPU_RUSTFLAGS "-Clink-arg=$<TARGET_FILE:SDL2::SDL2-static>")
    foreach(HP2_B5_SDL_LINK_ITEM IN LISTS HP2_SDL_STATIC_LINK_ITEMS)
        if(HP2_B5_SDL_LINK_ITEM MATCHES "^(-l|-Wl,)")
            list(APPEND HP2_B5_GPU_RUSTFLAGS "-Clink-arg=${HP2_B5_SDL_LINK_ITEM}")
        endif()
    endforeach()
    string(ASCII 31 HP2_B5_RUSTFLAG_SEPARATOR)
    list(JOIN HP2_B5_GPU_RUSTFLAGS "${HP2_B5_RUSTFLAG_SEPARATOR}" HP2_B5_GPU_RUSTFLAGS)
    if(NOT "$ENV{CARGO_ENCODED_RUSTFLAGS}" STREQUAL "")
        string(PREPEND HP2_B5_GPU_RUSTFLAGS "$ENV{CARGO_ENCODED_RUSTFLAGS}${HP2_B5_RUSTFLAG_SEPARATOR}")
    elseif(NOT "$ENV{RUSTFLAGS}" STREQUAL "")
        separate_arguments(HP2_B5_INHERITED_RUSTFLAGS UNIX_COMMAND "$ENV{RUSTFLAGS}")
        list(JOIN HP2_B5_INHERITED_RUSTFLAGS "${HP2_B5_RUSTFLAG_SEPARATOR}" HP2_B5_INHERITED_RUSTFLAGS)
        string(PREPEND HP2_B5_GPU_RUSTFLAGS "${HP2_B5_INHERITED_RUSTFLAGS}${HP2_B5_RUSTFLAG_SEPARATOR}")
    endif()
    hp2_add_behavior_test(actor_screenshot_vm_native_owner_contracts "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}"
        "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_screenshot_vm_native_owner_contracts/Cargo"
        "CARGO_ENCODED_RUSTFLAGS=${HP2_B5_GPU_RUSTFLAGS}"
        "${HP2_CARGO_EXECUTABLE}" test
        --offline
        "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-engine
        "--features=sdl,renderer-wgpu"
        --test=actor_screenshot_vm_native_owner_contracts
    )
    set_tests_properties(actor_screenshot_vm_native_owner_contracts PROPERTIES LABELS "runtime;data-none")
    if(HP2_TEST_DATA_PRESENT)
        hp2_add_behavior_test(display_command_routing "${CMAKE_COMMAND}" -E env
            "CARGO_HOME=${HP2_CARGO_HOME}"
            "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/display_command_routing/Cargo"
            "CARGO_ENCODED_RUSTFLAGS=${HP2_B5_GPU_RUSTFLAGS}"
            "HP2_CORE_DATA_ROOT=${HP2_TEST_DATA_ROOT}"
            "${HP2_CARGO_EXECUTABLE}" test --locked --offline
            "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
            --package=hp2-engine --features=sdl,renderer-wgpu --lib
            camera_owner::sdl_display_exec::tests -- --test-threads=1
        )
        set_tests_properties(display_command_routing PROPERTIES
            LABELS "integration;data-prototype;renderer" TIMEOUT 600)
    endif()
    hp2_add_behavior_test(display_fullscreen_gpu "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}"
        "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/display_fullscreen_gpu/Cargo"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline
        "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-wgpu --lib fullscreen_ -- --ignored --test-threads=1
    )
    set_tests_properties(display_fullscreen_gpu PROPERTIES
        LABELS "integration;data-none;renderer;wgpu" RESOURCE_LOCK hp2_gpu TIMEOUT 600)
    # Integration tests also build Cargo's game binary: link the same CMake-owned
    # SDK products and frameworks as the native-free game, not a second SDK build.
    set(HP2_GAME_TEST_LINK_FLAGS)
    foreach(HP2_GAME_TEST_LIBRARY IN ITEMS
            OpenAL::OpenAL archive_static zlibstatic hp2_bzip2 liblzma)
        list(APPEND HP2_GAME_TEST_LINK_FLAGS
            "-Clink-arg=$<TARGET_FILE:${HP2_GAME_TEST_LIBRARY}>")
    endforeach()
    list(APPEND HP2_GAME_TEST_LINK_FLAGS
        ${HP2_NFD_RUST_FRAMEWORK_FLAGS}
        "-lframework=QuartzCore" "-lframework=CoreGraphics" "-lframework=Metal"
        "-lframework=Foundation" "-lframework=CoreFoundation" "-lframework=AppKit"
        "-lframework=CoreText" "-lobjc" "-liconv"
        "-Clink-arg=-Wl,-rpath,$<TARGET_FILE_DIR:OpenAL::OpenAL>"
        "-Clink-arg=-mmacosx-version-min=${CMAKE_OSX_DEPLOYMENT_TARGET}")
    list(JOIN HP2_GAME_TEST_LINK_FLAGS "${HP2_B5_RUSTFLAG_SEPARATOR}" HP2_GAME_TEST_LINK_FLAGS)
    set(HP2_GAME_TEST_RUSTFLAGS
        "${HP2_B5_GPU_RUSTFLAGS}${HP2_B5_RUSTFLAG_SEPARATOR}${HP2_GAME_TEST_LINK_FLAGS}")
    hp2_add_behavior_test(game_boot_unwind_contracts "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}"
        "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/game_boot_unwind_contracts/Cargo"
        "CARGO_ENCODED_RUSTFLAGS=${HP2_GAME_TEST_RUSTFLAGS}"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline
        "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-game --bin=hp2_game_rust
        boot_services::unwind_tests::
        -- --test-threads=1 --nocapture
    )
    set_tests_properties(game_boot_unwind_contracts PROPERTIES
        LABELS "integration;data-none"
        FIXTURES_REQUIRED game_boot_unwind_source_order
    )
    hp2_add_behavior_test(no_startup_launcher_contracts "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/NoStartupLauncherTests.py"
        "$<TARGET_FILE:hp2_game_rust>"
    )
    set_tests_properties(no_startup_launcher_contracts PROPERTIES LABELS "fast;data-none")
    hp2_add_behavior_test(verification_audio_contracts "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}"
        "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/verification_audio_contracts/Cargo"
        "CARGO_ENCODED_RUSTFLAGS=${HP2_GAME_TEST_RUSTFLAGS}"
        "HP2_DETERMINISTIC_VERIFICATION=1"
        "${HP2_CARGO_EXECUTABLE}" test
        --locked
        --offline
        "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-audio
        --features=openal
        --lib verification::tests
        -- --test-threads=1
    )
    set_tests_properties(verification_audio_contracts PROPERTIES LABELS "runtime;data-none")
endif()

# Both tests exercise SDL through the CMake-owned static archive. Reuse the
# display command Cargo cache instead of allocating another dependency tree.
if(APPLE)
    set(HP2_AUDIT_SDL_RUSTFLAGS "${HP2_B5_GPU_RUSTFLAGS}")
elseif(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    get_target_property(_hp2_audit_sdl_dir SDL2::SDL2-static BINARY_DIR)
    execute_process(COMMAND sh "${_hp2_audit_sdl_dir}/sdl2-config" --static-libs
        RESULT_VARIABLE _hp2_audit_sdl_status OUTPUT_VARIABLE _hp2_audit_sdl_text OUTPUT_STRIP_TRAILING_WHITESPACE)
    if(NOT _hp2_audit_sdl_status EQUAL 0)
        message(FATAL_ERROR "Cannot resolve SDL static dependencies for audit tests")
    endif()
    separate_arguments(_hp2_audit_sdl_items UNIX_COMMAND "${_hp2_audit_sdl_text}")
    set(_hp2_audit_sdl_flags "-Clink-arg=$<TARGET_FILE:SDL2::SDL2-static>")
    foreach(_hp2_audit_item IN LISTS _hp2_audit_sdl_items)
        if(_hp2_audit_item MATCHES "^(-l|-Wl,|-pthread)" AND NOT _hp2_audit_item STREQUAL "-lSDL2")
            list(APPEND _hp2_audit_sdl_flags "-Clink-arg=${_hp2_audit_item}")
        endif()
    endforeach()
    string(ASCII 31 _hp2_audit_separator)
    list(JOIN _hp2_audit_sdl_flags "${_hp2_audit_separator}" HP2_AUDIT_SDL_RUSTFLAGS)
endif()
if(APPLE OR CMAKE_SYSTEM_NAME STREQUAL "Linux")
    hp2_add_behavior_test(stock_resolution_transaction_contracts "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}" "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/display_command_routing/Cargo"
        "CARGO_ENCODED_RUSTFLAGS=${HP2_AUDIT_SDL_RUSTFLAGS}"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-engine --features=sdl,renderer-wgpu --lib stock_resolution_transaction::tests -- --test-threads=1)
    set_tests_properties(stock_resolution_transaction_contracts PROPERTIES LABELS "fast;data-none")
    if(HP2_TEST_DATA_PRESENT)
        hp2_add_behavior_test(stock_resolution_lifecycle_vm_sdl "${CMAKE_COMMAND}" -E env
            "CARGO_HOME=${HP2_CARGO_HOME}" "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/display_command_routing/Cargo"
            "CARGO_ENCODED_RUSTFLAGS=${HP2_AUDIT_SDL_RUSTFLAGS}" "HP2_CORE_DATA_ROOT=${HP2_TEST_DATA_ROOT}"
            "${HP2_CARGO_EXECUTABLE}" test --locked --offline "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
            --package=hp2-engine --features=sdl,renderer-wgpu --lib stock_resolution_lifecycle::tests -- --test-threads=1)
        set_tests_properties(stock_resolution_lifecycle_vm_sdl PROPERTIES LABELS "fast;data-prototype")
    endif()
    hp2_add_behavior_test(stock_display_transition_contracts "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}" "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/display_command_routing/Cargo"
        "CARGO_ENCODED_RUSTFLAGS=${HP2_AUDIT_SDL_RUSTFLAGS}"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-engine --features=sdl,renderer-wgpu --lib stock_display_transition::tests -- --test-threads=1)
    set_tests_properties(stock_display_transition_contracts PROPERTIES LABELS "fast;data-none")
    hp2_add_behavior_test(platform_live_display_mode_sdl "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}" "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/display_command_routing/Cargo"
        "CARGO_ENCODED_RUSTFLAGS=${HP2_AUDIT_SDL_RUSTFLAGS}"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-platform --features=sdl --lib live_display_mode::tests -- --test-threads=1)
    set_tests_properties(platform_live_display_mode_sdl PROPERTIES LABELS "fast;data-none" TIMEOUT 120)
endif()
# Bounded policy characterization, initially qualified on macOS only.
if(APPLE)
    hp2_add_behavior_test(render_lifecycle_failure_contracts "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}"
        "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/render_lifecycle_failure_contracts/Cargo"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline
        "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-render --lib
        lifecycle::tests::every_lifecycle_failure_returns_its_error_and_stops_at_that_leaf
        -- --exact --test-threads=1
    )
    set_tests_properties(render_lifecycle_failure_contracts PROPERTIES LABELS "fast;data-none")
    hp2_add_behavior_test(render_pre_render_failure_contracts "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}"
        "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/render_pre_render_failure_contracts/Cargo"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline
        "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-render --lib
        pre_render::tests::every_pre_render_failure_returns_its_error_and_stops_at_that_leaf
        -- --exact --test-threads=1
    )
    set_tests_properties(render_pre_render_failure_contracts PROPERTIES LABELS "fast;data-none")
    hp2_add_behavior_test(render_precache_failure_contracts "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}"
        "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/render_precache_failure_contracts/Cargo"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline
        "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-render --lib
        precache::tests::precache_stops_at_each_failed_query_without_later_callbacks
        -- --exact --test-threads=1
    )
    set_tests_properties(render_precache_failure_contracts PROPERTIES LABELS "fast;data-none")
    hp2_add_behavior_test(platform_client_post_edit_contracts "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}"
        "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/platform_client_post_edit_contracts/Cargo"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline
        "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-platform --lib client_policy::client_post_edit_contracts::
        -- --test-threads=1
    )
    set_tests_properties(platform_client_post_edit_contracts PROPERTIES LABELS "fast;data-none")
    hp2_add_behavior_test(platform_viewport_capture_contracts "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}"
        "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/platform_viewport_capture_contracts/Cargo"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline
        "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-platform --lib viewport_policy::viewport_capture_contracts::
        -- --test-threads=1
    )
    set_tests_properties(platform_viewport_capture_contracts PROPERTIES LABELS "fast;data-none")
endif()
hp2_add_behavior_test(core_unix_file_manager "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_unix_file_manager/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    unix_file_manager_owner::tests
    --
    --test-threads=1
)
set_tests_properties(core_unix_file_manager PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(core_runtime_paths_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_runtime_paths_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    native_runtime_init::tests
    --
    --test-threads=1
)
set_tests_properties(core_runtime_paths_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(core_package_file_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_package_file_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    package_file_owner::tests
    --
    --test-threads=1
)
set_tests_properties(core_package_file_owner_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_color_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_color_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=actor_color_contracts
)
set_tests_properties(actor_color_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_file_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_file_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=actor_file_contracts
)
set_tests_properties(actor_file_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(package_map_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/package_map_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=package_map_contracts
)
set_tests_properties(package_map_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(path_scout_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/path_scout_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=path_scout_contracts
)
set_tests_properties(path_scout_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(path_creation_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/path_creation_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=path_creation_contracts
)
set_tests_properties(path_creation_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(path_maintenance_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/path_maintenance_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=path_maintenance_contracts
)
set_tests_properties(path_maintenance_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(path_build_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/path_build_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=path_build_contracts
)
set_tests_properties(path_build_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(object_lifecycle_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/object_lifecycle_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=object_lifecycle_contracts
)
set_tests_properties(object_lifecycle_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(object_gc_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/object_gc_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=object_gc_contracts
)
set_tests_properties(object_gc_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(object_storage_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/object_storage_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=object_storage_contracts
)
set_tests_properties(object_storage_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(bit_archive_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/bit_archive_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=bits_replay
)
set_tests_properties(bit_archive_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(level_summary_owner "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/level_summary_owner/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    --config=profile.test.opt-level=1
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=level_summary_owner
    --
    --test-threads=1
)
set_tests_properties(level_summary_owner PROPERTIES LABELS "integration;data-prototype")
hp2_add_behavior_test(model_owner "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/model_owner/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    --config=profile.test.opt-level=1
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=model_owner
    --
    --test-threads=1
)
set_tests_properties(model_owner PROPERTIES LABELS "integration;data-prototype")
hp2_add_behavior_test(animation_owner "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/animation_owner/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    --config=profile.test.opt-level=1
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=animation_owner
    --
    --test-threads=1
)
set_tests_properties(animation_owner PROPERTIES LABELS "integration;data-none")
hp2_add_behavior_test(spawn_owner "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/spawn_owner/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    --config=profile.test.opt-level=1
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=spawn_owner
    --
    --test-threads=1
)
set_tests_properties(spawn_owner PROPERTIES LABELS "integration;data-none")
hp2_add_behavior_test(mesh_owner "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/mesh_owner/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    --config=profile.test.opt-level=1
    --manifest-path=${CMAKE_SOURCE_DIR}/Cargo.toml
    --package=hp2-engine
    --test=mesh_owner
    --
    --test-threads=1
)
set_tests_properties(mesh_owner PROPERTIES LABELS "integration;data-prototype")
hp2_add_behavior_test(skeletal_owner "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/skeletal_owner/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    --config=profile.test.opt-level=1
    --manifest-path=${CMAKE_SOURCE_DIR}/Cargo.toml
    --package=hp2-engine
    --test=skeletal_owner
    --
    --test-threads=1
)
set_tests_properties(skeletal_owner PROPERTIES LABELS "integration;data-none")
hp2_add_behavior_test(palette_owner "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/palette_owner/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    --config=profile.test.opt-level=1
    --manifest-path=${CMAKE_SOURCE_DIR}/Cargo.toml
    --package=hp2-engine
    --test=palette_owner
    --
    --test-threads=1
)
set_tests_properties(palette_owner PROPERTIES LABELS "integration;data-none")
hp2_add_behavior_test(texture_owner "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/texture_owner/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    --config=profile.test.opt-level=1
    --manifest-path=${CMAKE_SOURCE_DIR}/Cargo.toml
    --package=hp2-engine
    --test=texture_owner
    --
    --test-threads=1
)
set_tests_properties(texture_owner PROPERTIES LABELS "integration;data-prototype")
hp2_add_behavior_test(font_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/font_owner_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=font_owner_contracts
)
set_tests_properties(font_owner_contracts PROPERTIES LABELS "integration;data-none")
hp2_add_behavior_test(sound_owner "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/sound_owner/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    --config=profile.test.opt-level=1
    --manifest-path=${CMAKE_SOURCE_DIR}/Cargo.toml
    --package=hp2-engine
    --test=sound_owner
    --
    --test-threads=1
)
set_tests_properties(sound_owner PROPERTIES LABELS "integration;data-prototype")
hp2_add_behavior_test(sound_stream_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/sound_stream_owner_contracts/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    --config=profile.test.opt-level=1
    --manifest-path=${CMAKE_SOURCE_DIR}/Cargo.toml
    --package=hp2-engine
    --lib
    sound_stream_owner_contracts
    --
    --test-threads=1
)
set_tests_properties(sound_stream_owner_contracts PROPERTIES LABELS "integration;data-prototype")
hp2_add_behavior_test(rust_core_runtime "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/rust_core_runtime/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    # Full Core.u traverses deeply linked fields; optimize stack slots while retaining test-profile checks.
    --config=profile.test.opt-level=1
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=core_runtime
    --
    --test-threads=1
)
set_tests_properties(rust_core_runtime PROPERTIES LABELS "integration;data-prototype")
hp2_add_behavior_test(canonical_engine_package_load "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/canonical_engine_package_load/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    --config=profile.test.opt-level=1
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=engine_package_load
    --
    --test-threads=1
)
set_tests_properties(canonical_engine_package_load PROPERTIES LABELS "integration;data-prototype")
hp2_add_behavior_test(actor_physics_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_physics_owner_contracts/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    --config=profile.test.opt-level=1
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --lib
    actor_physics_owner::tests
    --
    --test-threads=1
)
set_tests_properties(actor_physics_owner_contracts PROPERTIES LABELS "integration;data-prototype")
hp2_add_behavior_test(canvas_native_text_scalar_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/canvas_native_text_scalar_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    --config=profile.test.opt-level=1
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --lib
    render_device_owner::canvas_text_tchar_tests
    --
    --test-threads=1
)
set_tests_properties(canvas_native_text_scalar_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(renderer_light_manager_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/renderer_light_manager_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    --config=profile.test.opt-level=1
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --lib
    render_owner::light_manager_lifetime_tests
    --
    --test-threads=1
)
set_tests_properties(renderer_light_manager_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(renderer_host_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/renderer_host_contracts/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    --config=profile.test.opt-level=1
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --features=renderer-wgpu
    --lib
    render_host_contracts
    --
    --test-threads=1
)
set_tests_properties(renderer_host_contracts PROPERTIES LABELS "integration;data-prototype")
hp2_add_behavior_test(actor_sprite_hit_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_sprite_hit_owner_contracts/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    --config=profile.test.opt-level=1
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --features=renderer-wgpu
    --lib
    actor_sprite_hit_scopes_preserve_nested_viewport_ownership
    --
    --test-threads=1
)
set_tests_properties(actor_sprite_hit_owner_contracts PROPERTIES LABELS "integration;data-prototype")
hp2_add_behavior_test(fractal_owner_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/fractal_owner_contracts/Cargo"
    "HP2_CORE_DATA_ROOT=${HP2_UNREAL_ROOT}"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    --config=profile.test.opt-level=1
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --lib
    fractal_owner::tests
    --
    --test-threads=1
)
set_tests_properties(fractal_owner_contracts PROPERTIES LABELS "integration;data-prototype")
hp2_add_behavior_test(property_link_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/property_link_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=property_link_contracts
)
set_tests_properties(property_link_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(unix_signal_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/unix_signal_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=unix_signal_contracts
)
set_tests_properties(unix_signal_contracts PROPERTIES LABELS "integration;data-none")
hp2_add_behavior_test(vm_dispatch_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/vm_dispatch_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=vm_dispatch_contracts
)
set_tests_properties(vm_dispatch_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(vm_state_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/vm_state_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=vm_state_contracts
)
set_tests_properties(vm_state_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(class_metadata_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/class_metadata_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=class_metadata_contracts
)
set_tests_properties(class_metadata_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(class_lifecycle_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/class_lifecycle_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=class_lifecycle_contracts
)
set_tests_properties(class_lifecycle_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(class_storage_kernel_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/class_storage_kernel_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib class_
)
set_tests_properties(class_storage_kernel_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(class_registration_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/class_registration_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=class_registration_contracts
)
set_tests_properties(class_registration_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(object_serialization_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/object_serialization_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=object_serialization_contracts
)
set_tests_properties(object_serialization_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(property_construction_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/property_construction_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=property_construction_contracts
)
set_tests_properties(property_construction_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(property_layout_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/property_layout_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=property_layout_contracts
)
set_tests_properties(property_layout_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(expression_archive_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/expression_archive_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=expression_archive_contracts
)
set_tests_properties(expression_archive_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(metadata_script_tail_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/metadata_script_tail_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib core_metadata::tests::native_call_at_script_end_peeks_and_rewinds_without_widening_operand_bounds
)
set_tests_properties(metadata_script_tail_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(canonical_property_export_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/canonical_property_export_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib core_property_text::tests::full_export_preserves_default_comparison_and_data_delta_alias
)
set_tests_properties(canonical_property_export_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(platform_speech_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/platform_speech_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-platform
    --lib speech::tests
)
set_tests_properties(platform_speech_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(verification_input_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/verification_input_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-platform
    --lib input_pump::tests
)
set_tests_properties(verification_input_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(property_archive_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/property_archive_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=property_archive_contracts
)
set_tests_properties(property_archive_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(vm_scalar_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/vm_scalar_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=vm_scalar_contracts
)
set_tests_properties(vm_scalar_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(property_tag_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/property_tag_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=property_tag_contracts
)
set_tests_properties(property_tag_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(property_value_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/property_value_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=property_value_contracts
)
set_tests_properties(property_value_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(network_archive_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/network_archive_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=network_archive_contracts
)
set_tests_properties(network_archive_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(startup_defaults_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/startup_defaults_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=startup_defaults_contracts
)
set_tests_properties(startup_defaults_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(cpp_property_layout_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/cpp_property_layout_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=cpp_property_layout_contracts
)
set_tests_properties(cpp_property_layout_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(transaction_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/transaction_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib transaction::tests
)
set_tests_properties(transaction_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(vm_vector_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/vm_vector_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=vm_vector_contracts
)
set_tests_properties(vm_vector_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(property_text_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/property_text_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=property_text_contracts
)
set_tests_properties(property_text_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(vm_convert_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/vm_convert_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=vm_convert_contracts
)
set_tests_properties(vm_convert_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(vm_string_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/vm_string_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=vm_string_contracts
)
set_tests_properties(vm_string_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(vm_variable_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/vm_variable_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=vm_variable_contracts
)
set_tests_properties(vm_variable_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(vm_flow_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/vm_flow_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=vm_flow_contracts
)
set_tests_properties(vm_flow_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(vm_call_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/vm_call_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=vm_call_contracts
)
set_tests_properties(vm_call_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(vm_object_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/vm_object_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=vm_object_contracts
)
set_tests_properties(vm_object_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(vm_registry_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/vm_registry_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=vm_registry_contracts
)
set_tests_properties(vm_registry_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(vm_frame_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/vm_frame_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=vm_frame_contracts
)
set_tests_properties(vm_frame_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(vm_locals_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/vm_locals_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=vm_locals_contracts
)
set_tests_properties(vm_locals_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(array_storage_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/array_storage_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=array_storage_contracts
)
set_tests_properties(array_storage_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(object_collections_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/object_collections_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=object_collections_contracts
)
set_tests_properties(object_collections_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(string_storage_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/string_storage_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=string_storage_contracts
)
set_tests_properties(string_storage_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(map_storage_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/map_storage_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=map_storage_contracts
)
set_tests_properties(map_storage_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(object_properties_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/object_properties_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=object_properties_contracts
)
set_tests_properties(object_properties_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(object_construction_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/object_construction_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=object_construction_contracts
)
set_tests_properties(object_construction_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(package_file_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/package_file_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=package_file_contracts
)
set_tests_properties(package_file_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(file_persistence_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/file_persistence_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=file_persistence_contracts
)
set_tests_properties(file_persistence_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(file_stream_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/file_stream_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-audio
    --lib
    file_stream::tests
    --
    --test-threads=1
)
set_tests_properties(file_stream_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(mem_stack_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/mem_stack_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=mem_replay
    --test=mem_stack_contracts
)
set_tests_properties(mem_stack_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(scene_lighting_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/scene_lighting_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    -p hp2-render
    --test=scene_lighting_reentrancy
)
set_tests_properties(scene_lighting_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(linker_runtime_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/linker_runtime_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    linker_runtime::tests
)
set_tests_properties(linker_runtime_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(math_inline_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/math_inline_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    math::inline_contract_tests
)
set_tests_properties(math_inline_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(array_archive_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/array_archive_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    array_archive::tests
)
set_tests_properties(array_archive_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(package_flow_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/package_flow_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    package_flow::tests
)
set_tests_properties(package_flow_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(paths_bootstrap_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/paths_bootstrap_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-bootstrap-paths
    --lib
)
set_tests_properties(paths_bootstrap_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(runtime_init_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/runtime_init_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=runtime_init_contracts
)
set_tests_properties(runtime_init_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(core_name_replay "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_name_replay/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test=name_replay
)
set_tests_properties(core_name_replay PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(decal_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/decal_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=decal_contracts
)
set_tests_properties(decal_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_process_state_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_process_state_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=process_state_contracts
)
set_tests_properties(actor_process_state_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_remote_function_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_remote_function_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=actor_rpc_contracts
)
set_tests_properties(actor_remote_function_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_unactor_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_unactor_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=actor_unactor
)
set_tests_properties(actor_unactor_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(actor_is_animating "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/actor_is_animating/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    actor::animation_tests
)
set_tests_properties(actor_is_animating PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(engine_path_define_paths "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/engine_path_define_paths/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    path::tests::active_define_paths_connects_same_class_nodes_and_excludes_player_starts
    -- --exact
)
set_tests_properties(engine_path_define_paths PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(package_summary_archive "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/package_summary_archive/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    package_summary_archive::tests
)
set_tests_properties(package_summary_archive PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(class_dependency_archive "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/class_dependency_archive/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    class_dependency_archive::tests
)
set_tests_properties(class_dependency_archive PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(core_net_archive "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/core_net_archive/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    core_net_archive::tests
)
set_tests_properties(core_net_archive PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(linker_name_archive "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/linker_name_archive/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    linker_name_archive::tests
)
set_tests_properties(linker_name_archive PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(linker_record_archive "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/linker_record_archive/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    linker_record_archive::tests
)
set_tests_properties(linker_record_archive PROPERTIES LABELS "fast;data-none")

if(HP2_TEST_DATA_PRESENT)
    hp2_add_behavior_test(spell_interaction_manifest "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Build/spell_interaction_audit.py"
        "--repo-root=${PROJECT_SOURCE_DIR}"
        "--data-root=${HP2_TEST_DATA_ROOT}"
        "--output=${CMAKE_BINARY_DIR}/goldens/spell-interactions.json"
        --check
    )
    set_tests_properties(spell_interaction_manifest PROPERTIES
        WORKING_DIRECTORY "${CMAKE_BINARY_DIR}/Testing/HP2/spell_interaction_manifest"
    )
else()
    message(STATUS "Skipping data-backed tests: no System/Default.ini under ${HP2_TEST_DATA_ROOT}")
endif()
if(EXISTS "${PROJECT_SOURCE_DIR}/HarryPotter1/Unreal/System/Default.ini")
    add_custom_target(regenerate_goldens
        COMMAND "${CMAKE_COMMAND}" -E make_directory "${CMAKE_BINARY_DIR}/goldens"
        COMMAND "${Python3_EXECUTABLE}" "${PROJECT_SOURCE_DIR}/Build/package79_reference.py"
            "--repo-root=${PROJECT_SOURCE_DIR}"
            "--output=${CMAKE_BINARY_DIR}/goldens/package79-reference.json"
        COMMAND "${Python3_EXECUTABLE}" "${PROJECT_SOURCE_DIR}/Build/spell_interaction_audit.py"
            "--repo-root=${PROJECT_SOURCE_DIR}"
            "--data-root=${HP2_TEST_DATA_ROOT}"
            "--output=${CMAKE_BINARY_DIR}/goldens/spell-interactions.json"
        COMMAND "${Python3_EXECUTABLE}" "${PROJECT_SOURCE_DIR}/Build/package79_reference.py"
            "--data-root=${PROJECT_SOURCE_DIR}/HarryPotter1/Unreal"
            "--output=${CMAKE_BINARY_DIR}/goldens/hp1-package-audit.json"
        WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}"
        COMMENT "Regenerating machine-local goldens from the configured test data root")
else()
    add_custom_target(regenerate_goldens
        COMMAND "${CMAKE_COMMAND}" -E make_directory "${CMAKE_BINARY_DIR}/goldens"
        COMMAND "${Python3_EXECUTABLE}" "${PROJECT_SOURCE_DIR}/Build/package79_reference.py"
            "--repo-root=${PROJECT_SOURCE_DIR}"
            "--output=${CMAKE_BINARY_DIR}/goldens/package79-reference.json"
        COMMAND "${Python3_EXECUTABLE}" "${PROJECT_SOURCE_DIR}/Build/spell_interaction_audit.py"
            "--repo-root=${PROJECT_SOURCE_DIR}"
            "--data-root=${HP2_TEST_DATA_ROOT}"
            "--output=${CMAKE_BINARY_DIR}/goldens/spell-interactions.json"
        WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}"
        COMMENT "Regenerating machine-local goldens from the configured test data root (no HP1 data root present)")
endif()
if(HP2_TEST_DATA_PRESENT)
    hp2_add_behavior_test(spell_runtime_contracts "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}"
        "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/spell_runtime_contracts/Cargo"
        "HP2_CORE_DATA_ROOT=${HP2_TEST_DATA_ROOT}"
        "${HP2_CARGO_EXECUTABLE}" test
        --locked
        --offline
        "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-engine
        --test
        spell_runtime_contracts
        --
        --test-threads=1
    )
    set_tests_properties(spell_runtime_contracts PROPERTIES LABELS "integration;data-prototype")
endif()
hp2_add_behavior_test(dxt1_codec "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/dxt1_codec/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-codecs
    dxt1
    --
    --test-threads=1
)
hp2_add_behavior_test(eaxa_decoder "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/eaxa_decoder/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-codecs
    eaxa
    --
    --test-threads=1
)
hp2_add_behavior_test(replay_roundtrip "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/replay_roundtrip/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test
    net_replay
    --
    --test-threads=1
)
hp2_add_behavior_test(config_ini_parse_semantics "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/config_ini_parse_semantics/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    parse_semantics
    --
    --test-threads=1
)
hp2_add_behavior_test(config_ini_typed_getters "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/config_ini_typed_getters/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    typed_getters
    --
    --test-threads=1
)
hp2_add_behavior_test(config_ini_set_and_write "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/config_ini_set_and_write/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    set_and_write
    --
    --test-threads=1
)
hp2_add_behavior_test(config_ini_rewrite_normalization "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/config_ini_rewrite_normalization/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    rewrite_normalization
    --
    --test-threads=1
)
hp2_add_behavior_test(config_ini_cache_filenames "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/config_ini_cache_filenames/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    cache_filenames
    --
    --test-threads=1
)
hp2_add_behavior_test(config_ini_unicode_roundtrip "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/config_ini_unicode_roundtrip/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --lib
    unicode_roundtrip
    --
    --test-threads=1
)
hp2_add_behavior_test(audio_lifecycle "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/audio_lifecycle/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-audio
    --lib
    --
    --test-threads=1
)
hp2_add_behavior_test(input_edge_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/input_edge_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test
    input_uncon
    --
    --test-threads=1
)
hp2_add_behavior_test(input_axis_order "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/input_axis_order/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test
    input_uncon
    --
    --test-threads=1
)
hp2_add_behavior_test(input_release_all "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/input_release_all/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test
    input_uncon
    --
    --test-threads=1
)
hp2_add_behavior_test(render_lumos_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/render_lumos_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-render
    --lib
    lumos_contract_tests
    --
    --test-threads=1
)
set_tests_properties(render_lumos_contracts PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(render_console_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/render_console_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-render
    --lib
    console
    --
    --test-threads=1
)
hp2_add_behavior_test(paused_mouse_history_contract "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/paused_mouse_history_contract/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-engine
    --test=lev_tic_level_replay
    paused_level_clears_mouse_history_before_menu_tick_and_preserves_gameplay
    --
    --test-threads=1
)
set_tests_properties(paused_mouse_history_contract PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(mouse_state_round_trip "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/mouse_state_round_trip/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-platform
    --lib
    mouse_state
    --
    --test-threads=1
)
hp2_add_behavior_test(window_chrome_round_trip "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/window_chrome_round_trip/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-platform
    --lib
    window_chrome
    --
    --test-threads=1
)
hp2_add_behavior_test(joystick_round_trip "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/joystick_round_trip/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-platform
    --lib
    joystick
    --
    --test-threads=1
)
hp2_add_behavior_test(data_identity_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/data_identity_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-bootstrap-paths
    --lib
    --
    --test-threads=1
)
hp2_add_behavior_test(game_test_contract "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/GameTestTests.py"
)
hp2_add_behavior_test(port_progress_contract "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/PortProgressTests.py"
)
set_tests_properties(port_progress_contract PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(repair_save_contract "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/RepairSaveTests.py"
)
hp2_add_behavior_test(ucc_smoke_contract "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/UccSmokeTests.py"
)
if(EXISTS "${CMAKE_BINARY_DIR}/hp2_ucc_rust")
    hp2_add_behavior_test(ucc_help_smoke "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/UccSmokeTests.py"
        --smoke "--ucc-binary=${CMAKE_BINARY_DIR}/hp2_ucc_rust"
        "--data-root=${HP2_TEST_DATA_ROOT}"
    )
endif()
hp2_add_behavior_test(prototype_archive_contract "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/PrototypeArchiveTests.py"
)

# HP1 loader audit: the version-parameterized package reader in
# Build/package79_reference.py decodes every package under the HP1 data root
# (FileVersion 61-76, LicenseeVersion 0) and byte-verifies the machine-local
# audit golden under ${CMAKE_BINARY_DIR}/goldens (regenerate_goldens).
# Registration follows the renderer-smoke convention:
# gated at configure time on the HP1 data root; without it the test simply
# does not exist (a configuration gap, not a skip). The `data-hp1` label
# extends the OPERATIONS.md data-profile taxonomy; Docs gain it via
# Docs/HP1_MIGRATION.md Phase 1, not this file.
if(EXISTS "${PROJECT_SOURCE_DIR}/HarryPotter1/Unreal/System/Default.ini")
    hp2_add_behavior_test(hp1_loader_manifest "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Build/package79_reference.py"
        "--data-root=${PROJECT_SOURCE_DIR}/HarryPotter1/Unreal"
        "--output=${CMAKE_BINARY_DIR}/goldens/hp1-package-audit.json"
        --check
    )
    set_tests_properties(hp1_loader_manifest PROPERTIES LABELS "integration;data-hp1")
endif()
hp2_add_behavior_test(save_format_contract "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/SaveFormatTests.py"
)
if(EXISTS "${HP2_TEST_DATA_ROOT}/System/CUTSCENES")
    hp2_add_behavior_test(localization_contract "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/LocalizationTests.py"
        -v
        "--data-root=${HP2_TEST_DATA_ROOT}"
    )
endif()
if(EXISTS "${PROJECT_SOURCE_DIR}/dist/macos-arm64/HarryPotter2.app"
        AND EXISTS "${HP2_TEST_DATA_ROOT}/System/Default.ini")
    hp2_add_behavior_test(input_script_contract "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/InputScriptSmoke.py"
    )
    hp2_add_behavior_test(input_script_smoke "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/InputScriptSmoke.py"
        --smoke
        "--data-root=${HP2_TEST_DATA_ROOT}"
        --ticks=240
        --timeout=120
    )
    set_tests_properties(input_script_smoke PROPERTIES
        LABELS "integration;data-prototype"
        RESOURCE_LOCK hp2_gpu
    )
    hp2_add_behavior_test(canonical_input_script_smoke "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/InputScriptSmoke.py"
        --smoke
        "--data-root=${HP2_TEST_DATA_ROOT}"
        --ticks=240
        --timeout=120
        "--engine-bin=$<TARGET_FILE:hp2_game_rust>"
    )
    set_tests_properties(canonical_input_script_smoke PROPERTIES
        LABELS "integration;data-prototype"
        RESOURCE_LOCK hp2_gpu
    )
    hp2_add_behavior_test(canonical_save_reload_smoke "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/InputScriptSmoke.py"
        --smoke
        "--data-root=${HP2_TEST_DATA_ROOT}"
        --engine-bin=$<TARGET_FILE:hp2_game_rust>
        --map=Adv1Willow
        --fixture=Tests/Fixtures/input_script_save_reload.txt
        --bind "F5=SAVEGAME 9"
        --bind "F9=LOADGAME 9"
        --expect-user-file=Save9.usa
        --expect-map-load=Save9.usa
        --ticks=300
        --timeout=180
    )
    set_tests_properties(canonical_save_reload_smoke PROPERTIES
        LABELS "integration;data-prototype"
        RESOURCE_LOCK hp2_gpu
    )
endif()
hp2_add_behavior_test(framing_baseline_contract "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/FramingBaselineTests.py"
)
set_tests_properties(framing_baseline_contract PROPERTIES LABELS "fast;data-none")

if(EXISTS "${PROJECT_SOURCE_DIR}/dist/macos-arm64/HarryPotter2.app"
        AND EXISTS "${HP2_TEST_DATA_ROOT}/System/Default.ini")
    hp2_add_behavior_test(determinism_double_run "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/DeterminismDoubleRunTests.py"
        --app "${CMAKE_BINARY_DIR}/hp2_game_rust"
        --ticks=300
    )
    set_tests_properties(determinism_double_run PROPERTIES
        LABELS "integration;data-prototype"
        RESOURCE_LOCK hp2_gpu
    )
endif()


hp2_add_behavior_test(native_typography_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/native_typography_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-render
    --lib
    native_text_layout
    --
    --test-threads=1
)
hp2_add_behavior_test(object_registry_contracts "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/object_registry_contracts/Cargo"
    "${HP2_CARGO_EXECUTABLE}" test
    --locked
    --offline
    "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
    --package=hp2-core
    --test
    class_registration_contracts
    --
    --test-threads=1
)

if(EXISTS "${HP2_UNREAL_ROOT}/System/Engine.u"
        AND EXISTS "${HP2_UNREAL_ROOT}/System/HGame.u"
        AND EXISTS "${HP2_UNREAL_ROOT}/System/UWindow.u")
    hp2_add_behavior_test(canvas_compatibility_contracts "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}"
        "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/canvas_compatibility_contracts/Cargo"
        "HP2_CORE_DATA_ROOT=${HP2_TEST_DATA_ROOT}"
        "${HP2_CARGO_EXECUTABLE}" test
        --locked
        --offline
        "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-engine
        --test
        canvas_compatibility_contracts
        --
        --test-threads=1
    )
    set_tests_properties(canvas_compatibility_contracts
        PROPERTIES LABELS "integration;data-prototype"
    )
endif()

# Deterministic renderer smoke tiers. The commit gate runs a fixed three-map
# subset; the full sweep is registered under the smoke-full label and
# excluded from default test presets because it launches every playable map.
# Both require the installed app bundle and prototype data at configure time;
# without them the tests are not silently skipped, they simply do not exist,
# which the verification docs call out as a configuration gap.
if(EXISTS "${PROJECT_SOURCE_DIR}/dist/macos-arm64/HarryPotter2.app"
        AND EXISTS "${HP2_TEST_DATA_ROOT}/System/Default.ini")
    hp2_add_behavior_test(renderer_smoke_maps "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Build/smoke_maps.py"
        "--renderer=wgpu"
        "--app=${CMAKE_BINARY_DIR}/hp2_game_rust"
        "--maps=PrivetDr,Entry,Ch2Skurge"
        "--ticks=120"
        "--timeout=90"
        "--output=${CMAKE_BINARY_DIR}/Testing/HP2/renderer_smoke_maps/smoke-maps.json"
        "--data-root=${HP2_TEST_DATA_ROOT}"
    )
    if(HP2_ENABLE_FULL_MAP_SMOKE)
        hp2_add_behavior_test(renderer_smoke_full "${Python3_EXECUTABLE}"
            "${PROJECT_SOURCE_DIR}/Build/smoke_maps.py"
            "--renderer=wgpu"
            "--app=${CMAKE_BINARY_DIR}/hp2_game_rust"
            "--ticks=300"
            "--output=${CMAKE_BINARY_DIR}/Testing/HP2/renderer_smoke_full/smoke-maps.json"
            "--data-root=${HP2_TEST_DATA_ROOT}"
        )
        set_tests_properties(renderer_smoke_full PROPERTIES
            LABELS "smoke;renderer;wgpu;smoke-full"
            RESOURCE_LOCK hp2_gpu
            TIMEOUT 14400
        )
    endif()
endif()

if(HP2_TEST_DATA_PRESENT AND TARGET hp2_game_rust)
    hp2_add_behavior_test(canonical_game_boot_entry "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Build/smoke_maps.py"
        "--renderer=wgpu"
        "--app=$<TARGET_FILE:hp2_game_rust>"
        "--maps=Entry"
        "--ticks=120"
        "--timeout=90"
        "--output=${CMAKE_BINARY_DIR}/Testing/HP2/canonical_game_boot_entry/Artifacts/smoke-maps.json"
        "--data-root=${HP2_TEST_DATA_ROOT}"
    )
    hp2_add_behavior_test(canonical_renderer_smoke_maps "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Build/smoke_maps.py"
        "--renderer=wgpu"
        "--app=$<TARGET_FILE:hp2_game_rust>"
        "--maps=PrivetDr,Entry,Ch2Skurge"
        "--ticks=120"
        "--timeout=90"
        "--output=${CMAKE_BINARY_DIR}/Testing/HP2/canonical_renderer_smoke_maps/Artifacts/smoke-maps.json"
        "--data-root=${HP2_TEST_DATA_ROOT}"
    )
    set_tests_properties(canonical_game_boot_entry canonical_renderer_smoke_maps PROPERTIES
        LABELS "integration;data-prototype"
        RESOURCE_LOCK hp2_gpu
        TIMEOUT 360
    )
endif()

# Full authored-checkpoint parity is intentionally macOS arm64-only: the
# independent oracle bundle and Rust game target must both be real artifacts.
if(APPLE
        AND CMAKE_SYSTEM_PROCESSOR MATCHES "^(arm64|aarch64)$"
        AND HP2_TEST_DATA_PRESENT
        AND TARGET hp2_game_rust
        AND EXISTS "${HP2_CPP_ORACLE_APP}")
    hp2_add_behavior_test(parity_paired_maps "${CMAKE_COMMAND}"
        "-DHP2_PYTHON=${Python3_EXECUTABLE}"
        "-DHP2_SOURCE_DIR=${PROJECT_SOURCE_DIR}"
        "-DHP2_ORACLE_APP=${HP2_CPP_ORACLE_APP}"
        "-DHP2_ENGINE_BIN=$<TARGET_FILE:hp2_game_rust>"
        "-DHP2_DATA_ROOT=${HP2_TEST_DATA_ROOT}"
        "-DHP2_OUTPUT_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/parity_paired_maps/Artifacts"
        -P "${PROJECT_SOURCE_DIR}/Build/parity_paired_maps.cmake"
    )
    set_tests_properties(parity_paired_maps PROPERTIES
        LABELS "integration;data-prototype;parity-full"
        RESOURCE_LOCK hp2_gpu
        TIMEOUT 14400
    )
endif()

# Label taxonomy: layer labels (fast|integration|smoke|visual|manual) plus a
# data-profile label (data-none|data-prototype|data-retail). Smoke tests keep
# the labels assigned in the smoke block above.
set_tests_properties(command_line_load
    compact_index fstring_archive ftime_archive dxt1_codec eaxa_decoder
    game_test_contract repair_save_contract ucc_smoke_contract prototype_archive_contract
    PROPERTIES LABELS "fast;data-none"
)
if(HP2_TEST_DATA_PRESENT)
    set_tests_properties(spell_interaction_manifest spell_runtime_contracts
        PROPERTIES LABELS "integration;data-prototype"
    )
endif()
if(APPLE)
        hp2_add_behavior_test(wgpu_draw_arena_gpu "${CMAKE_COMMAND}" -E env
            "CARGO_HOME=${HP2_CARGO_HOME}"
            "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/wgpu_surface_round_trip/Cargo"
            "${HP2_CARGO_EXECUTABLE}" test
            --locked --offline
            "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
            --package=hp2-wgpu --lib draw_arena_
            -- --ignored --test-threads=1
        )
        set_tests_properties(wgpu_draw_arena_gpu PROPERTIES
            LABELS "integration;data-none;renderer;wgpu"
            RESOURCE_LOCK hp2_gpu
            TIMEOUT 600
        )
        hp2_add_behavior_test(wgpu_lumos_readback "${CMAKE_COMMAND}" -E env
            "CARGO_HOME=${HP2_CARGO_HOME}"
            "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/wgpu_surface_round_trip/Cargo"
            "${HP2_CARGO_EXECUTABLE}" test
            --locked --offline
            "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
            --package=hp2-wgpu --lib lumos_
            -- --ignored --test-threads=1
        )
        set_tests_properties(wgpu_lumos_readback PROPERTIES
            LABELS "integration;data-none;renderer;wgpu"
            RESOURCE_LOCK hp2_gpu
            TIMEOUT 600
        )
        # The Metal surface round-trip that was Tests/WgpuSurfaceTests.cpp is
        # exercised by renderer_smoke_wgpu below; the pure-Rust wgpu unit
        # tests cover the capture/texture/parse contracts.
        hp2_add_behavior_test(wgpu_surface_round_trip "${CMAKE_COMMAND}" -E env
            "CARGO_HOME=${HP2_CARGO_HOME}"
            "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/wgpu_surface_round_trip/Cargo"
            "${HP2_CARGO_EXECUTABLE}" test
            --locked
            --offline
            "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
            --package=hp2-wgpu
            --lib
            --
            --test-threads=1
        )
        hp2_add_behavior_test(texture_p8_bytes_contracts "${CMAKE_COMMAND}" -E env
            "CARGO_HOME=${HP2_CARGO_HOME}"
            "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/texture_p8_bytes_contracts/Cargo"
            "${HP2_CARGO_EXECUTABLE}" test --locked --offline
            "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
            --package=hp2-wgpu --lib
            texture::tests::p8_owned_bytes_match_reference_and_expected_texels
            -- --exact --test-threads=1
        )
        set_tests_properties(texture_p8_bytes_contracts PROPERTIES LABELS "fast;data-none")
        # Explicitly execute the ignored device test. Missing GPU support is a
        # failure, not a skip. This is readback, not real surface presentation.
        hp2_add_behavior_test(texture_p8_upload_round_trip "${CMAKE_COMMAND}" -E env
            "CARGO_HOME=${HP2_CARGO_HOME}"
            "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/texture_p8_upload_round_trip/Cargo"
            "${HP2_CARGO_EXECUTABLE}" test --locked --offline
            "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
            --package=hp2-wgpu --lib
            texture::tests::p8_owned_bytes_upload_round_trips_through_render_loop
            -- --ignored --exact --test-threads=1
        )
        set_tests_properties(texture_p8_upload_round_trip PROPERTIES
            LABELS "integration;data-none;renderer;wgpu"
            RESOURCE_LOCK hp2_gpu
            TIMEOUT 600
        )
        hp2_add_behavior_test(engine_p8_upload_contracts "${CMAKE_COMMAND}" -E env
            "CARGO_HOME=${HP2_CARGO_HOME}"
            "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/engine_p8_upload_contracts/Cargo"
            "${HP2_CARGO_EXECUTABLE}" test --locked --offline
            "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
            --package=hp2-engine --features=renderer-wgpu --lib
            render_device_owner::p8_upload_tests::engine_p8_upload_policy_cache_and_errors
            -- --ignored --exact --test-threads=1
        )
        set_tests_properties(engine_p8_upload_contracts PROPERTIES
            LABELS "integration;data-none;renderer;wgpu"
            RESOURCE_LOCK hp2_gpu
            TIMEOUT 600
        )
    endif()
    hp2_add_behavior_test(wgpu_world_scale_gpu "${CMAKE_COMMAND}" -E env
        "CARGO_HOME=${HP2_CARGO_HOME}"
        "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/wgpu_surface_round_trip/Cargo"
        "${HP2_CARGO_EXECUTABLE}" test --locked --offline
        "--manifest-path=${PROJECT_SOURCE_DIR}/Cargo.toml"
        --package=hp2-wgpu --lib world_scale
        -- --include-ignored --test-threads=1
    )
    set_tests_properties(wgpu_world_scale_gpu PROPERTIES
        LABELS "integration;data-none;renderer;wgpu"
        RESOURCE_LOCK hp2_gpu
        TIMEOUT 600
    )
    find_package(Python3 REQUIRED COMPONENTS Interpreter)
    # hp2_add_behavior_test (not a bare add_test): bare add_test sets no
    # HP2_ARTIFACT_DIR, so run_wgpu_smoke.py's own HP2_ARTIFACT_DIR
    # fallback (a fresh tempfile.mkdtemp()) would kick in instead,
    # and its log/report would never reach the per-test artifact
    # directory -- AGENTS.md Non-Negotiable #2 requires exactly that
    # path (caught on review).
    hp2_add_behavior_test(renderer_smoke_wgpu "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Build/run_wgpu_smoke.py"
        "--build-dir=${CMAKE_BINARY_DIR}"
        "--data-root=${HP2_TEST_DATA_ROOT}"
        "--ticks=300"
    )
    set_tests_properties(renderer_smoke_wgpu PROPERTIES
        LABELS "smoke;renderer;wgpu"
        RESOURCE_LOCK hp2_gpu
        TIMEOUT 600
        # A 60-tick run on the macOS test host produced zero presented frames
        # and 60 occluded frames; an observed 300-tick run presented 299.
        # Keep the longer window so SDL/Metal can expose the surface before
        # judging presentation, but retain the hard requirement: a clean boot
        # with no presented frames remains blocked, never skipped.
    )
