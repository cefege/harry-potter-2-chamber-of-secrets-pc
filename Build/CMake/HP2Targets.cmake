# Root of the immutable prototype data tree handed to behavioral tests via
# -datadir/--data-root. Defaults to the build's prototype checkout so test
# runs can be pointed at an alternate data root without editing this file.
set(HP2_TEST_DATA_ROOT "${HP2_UNREAL_ROOT}" CACHE PATH
    "Prototype data root passed to behavioral tests via -datadir/--data-root")

add_library(hp2_compile_policy INTERFACE)
target_compile_features(hp2_compile_policy INTERFACE cxx_std_17)

target_compile_definitions(hp2_compile_policy INTERFACE
    __UNIX__=1
    MACOSX=1
    ASM=0
    UNICODE=1
    _UNICODE=1
    HP2_HAS_NATIVE_TEXT_BACKEND=$<BOOL:${HP2_HAS_NATIVE_TEXT_BACKEND}>
)

if(CMAKE_CXX_COMPILER_ID MATCHES "^(AppleClang|Clang)$")
    target_compile_options(hp2_compile_policy INTERFACE
        -Wall
        -Wextra
        # The VC6-era headers intentionally expose empty virtual defaults,
        # placement-new forms, and null-base reflected offset expressions.
        # Keep host-width conversions fatal while silencing that known noise.
        -Wno-unused-parameter
        -Wno-reorder-ctor
        -Wno-inline-new-delete
        -Wno-implicit-exception-spec-mismatch
        -Wno-deprecated-copy
        -Wno-exceptions
        -Wno-ignored-qualifiers
        -Wno-sign-compare
        -Wno-overloaded-virtual
        -Wno-extra-tokens
        -Wno-string-plus-int
        -Wno-null-pointer-subtraction
        -Werror=shorten-64-to-32
        -Werror=pointer-to-int-cast
        -Werror=int-to-pointer-cast
        "$<$<CONFIG:Release>:-O3>"
    )
endif()

if(HP2_ENABLE_ASAN_UBSAN)
    target_compile_options(hp2_compile_policy INTERFACE
        -fsanitize=address,undefined
        # UE1's GNU ABI deliberately places host-pointer containers on
        # four-byte package boundaries. Apple arm64 supports those accesses;
        # retain every other ASan/UBSan check without misdiagnosing that ABI.
        -fno-sanitize=alignment
        -fno-omit-frame-pointer
        -fno-sanitize-recover=all
    )
    target_link_options(hp2_compile_policy INTERFACE
        -fsanitize=address,undefined
        -fno-omit-frame-pointer
        -fno-sanitize-recover=all
    )
elseif(HP2_ENABLE_TSAN)
    target_compile_options(hp2_compile_policy INTERFACE
        -fsanitize=thread
        -fno-omit-frame-pointer
    )
    target_link_options(hp2_compile_policy INTERFACE
        -fsanitize=thread
        -fno-omit-frame-pointer
    )
endif()

set(HP2_EMPTY_MODULE_API_DEFINITIONS
    CORE_API=
    ENGINE_API=
    RENDER_API=
    FIRE_API=
    EDITOR_API=
    ALAUDIO_API=
    SDLDRV_API=
    XOPENGLDRV_API=
)

set(HP2_CORE_INCLUDE_DIRS
    "${HP2_UNREAL_ROOT}/Core/Inc"
    "${HP2_UNREAL_ROOT}/Core/Src"
)
set(HP2_ENGINE_INCLUDE_DIRS
    ${HP2_CORE_INCLUDE_DIRS}
    "${HP2_UNREAL_ROOT}/Engine/Inc"
    "${HP2_UNREAL_ROOT}/Engine/Src"
)
set(HP2_RENDER_INCLUDE_DIRS
    ${HP2_ENGINE_INCLUDE_DIRS}
    "${HP2_UNREAL_ROOT}/Render/Inc"
    "${HP2_UNREAL_ROOT}/Render/Src"
)
set(HP2_FIRE_INCLUDE_DIRS
    ${HP2_ENGINE_INCLUDE_DIRS}
    "${HP2_UNREAL_ROOT}/Fire/Inc"
    "${HP2_UNREAL_ROOT}/Fire/Src"
)
set(HP2_EDITOR_INCLUDE_DIRS
    ${HP2_ENGINE_INCLUDE_DIRS}
    "${HP2_UNREAL_ROOT}/Editor/Inc"
    "${HP2_UNREAL_ROOT}/Editor/Src"
)

set(HP2_ALAUDIO_INCLUDE_DIRS
    ${HP2_ENGINE_INCLUDE_DIRS}
    "${HP2_UNREAL_ROOT}/ALAudio/Inc"
    "${HP2_UNREAL_ROOT}/ALAudio/Src"
)
set(HP2_SDLDRV_INCLUDE_DIRS
    ${HP2_ENGINE_INCLUDE_DIRS}
    "${HP2_UNREAL_ROOT}/SDLDrv/Inc"
)
set(HP2_XOPENGLDRV_INCLUDE_DIRS
    ${HP2_RENDER_INCLUDE_DIRS}
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Inc"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/glm"
)
set(HP2_LAUNCH_INCLUDE_DIRS
    ${HP2_RENDER_INCLUDE_DIRS}
    ${HP2_FIRE_INCLUDE_DIRS}
    ${HP2_ALAUDIO_INCLUDE_DIRS}
    ${HP2_SDLDRV_INCLUDE_DIRS}
    ${HP2_XOPENGLDRV_INCLUDE_DIRS}
    "${HP2_UNREAL_ROOT}/Launch/Src"
    "${HP2_UNREAL_ROOT}/UCC/Src"
    "${HP2_UNREAL_ROOT}/SDLLaunch/Src"
    "${PROJECT_SOURCE_DIR}/Tests"
)
list(REMOVE_DUPLICATES HP2_LAUNCH_INCLUDE_DIRS)

function(hp2_add_engine_object target package_name)
    add_library("${target}" OBJECT ${ARGN})
    target_link_libraries("${target}" PRIVATE hp2_compile_policy)
    target_compile_definitions("${target}" PRIVATE
        __STATIC_LINK=1
        ${HP2_EMPTY_MODULE_API_DEFINITIONS}
        "GPackage=LocalPackageName${package_name}"
    )
    set_target_properties("${target}" PROPERTIES
        CXX_STANDARD 17
        CXX_STANDARD_REQUIRED YES
        CXX_EXTENSIONS NO
    )
endfunction()

hp2_add_engine_object(hp2_core Core ${HP2_CORE_SOURCES})
target_include_directories(hp2_core PUBLIC ${HP2_CORE_INCLUDE_DIRS})
target_link_libraries(hp2_core PUBLIC vorbisfile)

hp2_add_engine_object(hp2_engine Engine ${HP2_ENGINE_SOURCES})
target_include_directories(hp2_engine PUBLIC ${HP2_ENGINE_INCLUDE_DIRS})
target_include_directories(hp2_engine PRIVATE "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Inc")
target_link_libraries(hp2_engine PUBLIC Squish::Squish)

hp2_add_engine_object(hp2_render Render ${HP2_RENDER_SOURCES})
target_include_directories(hp2_render PUBLIC ${HP2_RENDER_INCLUDE_DIRS})
target_link_libraries(hp2_render PUBLIC Squish::Squish)

hp2_add_engine_object(hp2_fire Fire ${HP2_FIRE_SOURCES})
target_include_directories(hp2_fire PUBLIC ${HP2_FIRE_INCLUDE_DIRS})

hp2_add_engine_object(hp2_editor_runtime Editor ${HP2_EDITOR_RUNTIME_SOURCES})
target_include_directories(hp2_editor_runtime PRIVATE ${HP2_EDITOR_INCLUDE_DIRS})

hp2_add_engine_object(hp2_alaudio ALAudio ${HP2_ALAUDIO_SOURCES})
target_include_directories(hp2_alaudio PUBLIC ${HP2_ALAUDIO_INCLUDE_DIRS})
target_link_libraries(hp2_alaudio PUBLIC OpenAL::OpenAL vorbisfile)

hp2_add_engine_object(hp2_sdldrv SDLDrv ${HP2_SDLDRV_SOURCES})
target_include_directories(hp2_sdldrv PUBLIC ${HP2_SDLDRV_INCLUDE_DIRS})
target_link_libraries(hp2_sdldrv PUBLIC SDL2::SDL2-static)

hp2_add_engine_object(hp2_xopengldrv XOpenGLDrv ${HP2_XOPENGLDRV_SOURCES})
target_include_directories(hp2_xopengldrv PUBLIC ${HP2_XOPENGLDRV_INCLUDE_DIRS})
target_compile_definitions(hp2_xopengldrv PRIVATE
    XOPENGL_HP2=1
    XOPENGL_USE_SDL=1
    SDL2BUILD=1
)
target_link_libraries(hp2_xopengldrv PUBLIC
    SDL2::SDL2-static
    OpenGL::GL
    Squish::Squish
)

if(HP2_HAS_NATIVE_TEXT_BACKEND)
    # Driver-neutral CoreText shaping/rasterization core (NativeTextShared.*)
    # compiled once and consumed as objects by every render driver with a
    # native text leg, so the provider link libraries reach both consumers.
    hp2_add_engine_object(hp2_nativetextcore XOpenGLDrv
        "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/NativeTextShared.cpp"
    )
    target_include_directories(hp2_nativetextcore PUBLIC ${HP2_XOPENGLDRV_INCLUDE_DIRS})
    target_link_libraries(hp2_nativetextcore PUBLIC ${HP2_TEXT_PROVIDER_LINK_LIBS})

    # Link libraries exported by the selected native text provider
    # (HP2Dependencies.cmake), not provider-specific framework paths.
    target_link_libraries(hp2_xopengldrv PUBLIC
        ${HP2_TEXT_PROVIDER_LINK_LIBS}
        hp2_nativetextcore
    )
endif()

function(hp2_add_executable target)
    add_executable("${target}" ${ARGN})
    target_include_directories("${target}" PRIVATE ${HP2_LAUNCH_INCLUDE_DIRS})
    target_link_libraries("${target}" PRIVATE hp2_compile_policy)
    target_compile_definitions("${target}" PRIVATE
        __STATIC_LINK=1
        ${HP2_EMPTY_MODULE_API_DEFINITIONS}
    )
    set_target_properties("${target}" PROPERTIES
        CXX_STANDARD 17
        CXX_STANDARD_REQUIRED YES
        CXX_EXTENSIONS NO
    )
endfunction()

# Linking object-library targets directly preserves every registration object
# and propagates the required static dependency usage to each executable.
hp2_add_executable(hp2_game
    ${HP2_SDL_LAUNCH_SOURCES}
    ${HP2_STATIC_PACKAGE_SOURCE}
    ${HP2_PATH_SOURCE}
)
target_compile_definitions(hp2_game PRIVATE HP2_WITH_CLIENT_PACKAGES=1)
set_source_files_properties("${HP2_NATIVE_LAUNCHER_SOURCE}" PROPERTIES
    COMPILE_OPTIONS "-fobjc-arc;-std=c++17"
)
target_link_libraries(hp2_game PRIVATE
    hp2_core
    hp2_engine
    hp2_render
    hp2_fire
    hp2_editor_runtime
    hp2_alaudio
    hp2_sdldrv
    hp2_xopengldrv
    "${HP2_APPKIT_FRAMEWORK}"
)

if(HP2_HAS_NATIVE_TEXT_BACKEND)
    # Object libraries do not propagate their objects through other object
    # libraries: consume the shared native-text core directly wherever
    # hp2_xopengldrv is linked (mirrors the hp2_zvulkan idiom).
    target_sources(hp2_game PRIVATE $<TARGET_OBJECTS:hp2_nativetextcore>)
endif()

hp2_add_executable(hp2_ucc
    ${HP2_UCC_SOURCES}
    ${HP2_STATIC_PACKAGE_SOURCE}
    ${HP2_PATH_SOURCE}
)
target_link_libraries(hp2_ucc PRIVATE
    hp2_core
    hp2_engine
    hp2_render
    hp2_fire
    hp2_editor_runtime
)

hp2_add_executable(hp2_package_audit
    ${HP2_PACKAGE_AUDIT_SOURCE}
    ${HP2_STATIC_PACKAGE_SOURCE}
    ${HP2_PATH_SOURCE}
)
target_link_libraries(hp2_package_audit PRIVATE
    hp2_core
    hp2_engine
    hp2_render
    hp2_fire
    hp2_editor_runtime
)

hp2_add_executable(hp2_spell_runtime_tests
    ${HP2_SPELL_RUNTIME_TEST_SOURCE}
    ${HP2_STATIC_PACKAGE_SOURCE}
    ${HP2_PATH_SOURCE}
)
target_link_libraries(hp2_spell_runtime_tests PRIVATE
    hp2_core
    hp2_engine
    hp2_render
    hp2_fire
    hp2_editor_runtime
    hp2_sdldrv
)

hp2_add_executable(hp2_abi_tests
    ${HP2_ABI_TEST_SOURCE}
    ${HP2_STATIC_PACKAGE_SOURCE}
)
target_link_libraries(hp2_abi_tests PRIVATE
    hp2_core
    hp2_engine
    hp2_render
    hp2_fire
    hp2_editor_runtime
    hp2_sdldrv
)

hp2_add_executable(hp2_dxt1_tests
    ${HP2_DXT1_TEST_SOURCE}
    "${HP2_UNREAL_ROOT}/Engine/Src/S3tcCompat.cpp"
)
target_include_directories(hp2_dxt1_tests PRIVATE ${HP2_ENGINE_INCLUDE_DIRS})
target_link_libraries(hp2_dxt1_tests PRIVATE Squish::Squish)

# The decoder test deliberately compiles only the decoder unit rather than the
# full Core object library.
hp2_add_executable(hp2_eaxa_tests
    ${HP2_EAXA_TEST_SOURCE}
    "${HP2_UNREAL_ROOT}/Core/Src/FEAXABlockDecoder.cpp"
)
target_include_directories(hp2_eaxa_tests PRIVATE ${HP2_CORE_INCLUDE_DIRS})

hp2_add_executable(hp2_audio_tests
    ${HP2_AUDIO_TEST_SOURCE}
)
target_link_libraries(hp2_audio_tests PRIVATE
    hp2_core
    hp2_engine
    hp2_alaudio
)

hp2_add_executable(hp2_launcher_tests
    ${HP2_LAUNCHER_CORE_SOURCES}
    ${HP2_PATH_SOURCE}
    ${HP2_LAUNCHER_TEST_SOURCE}
)
target_compile_definitions(hp2_launcher_tests PRIVATE HP2_LAUNCHER_TESTING=1)
target_link_libraries(hp2_launcher_tests PRIVATE hp2_core)

# Config contract: links full hp2_core because FConfigCacheIni behavior lives
# in CORE_API helpers (appStricmp, codecs), not the header-only container.
hp2_add_executable(hp2_config_ini_tests
    ${HP2_CONFIG_INI_TEST_SOURCE}
)
target_link_libraries(hp2_config_ini_tests PRIVATE hp2_core)

# Replay wire-format contract: links the same dependency-light production
# operator translation unit as hp2_engine, without pulling UnReplay.cpp's
# appInit-dependent FURL globals into the oracle process.
hp2_add_executable(hp2_replay_roundtrip_tests
    ${HP2_REPLAY_ROUNDTRIP_TEST_SOURCE}
    "${HP2_UNREAL_ROOT}/Engine/Src/UnReplayWire.cpp"
)
target_link_libraries(hp2_replay_roundtrip_tests PRIVATE hp2_core)

# Production save-wire oracle. This is intentionally not a CTest: fixture
# generation and acceptance are separate explicit operations.
hp2_add_executable(hp2_save_wire_oracle
    ${HP2_SAVE_WIRE_ORACLE_SOURCE}
)
target_link_libraries(hp2_save_wire_oracle PRIVATE hp2_core)

# Input freeze + mouse-capture policy + normalization mapping contracts.
hp2_add_executable(hp2_input_contract_tests
    ${HP2_INPUT_CONTRACT_TEST_SOURCE}
    ${HP2_STATIC_PACKAGE_SOURCE}
    ${HP2_PATH_SOURCE}
)
target_link_libraries(hp2_input_contract_tests PRIVATE
    hp2_core
    hp2_engine
    hp2_render
    hp2_fire
    hp2_editor_runtime
    hp2_sdldrv
)
if(HP2_HAS_NATIVE_TEXT_BACKEND)
    hp2_add_executable(hp2_native_typography_tests
        ${HP2_NATIVE_TYPOGRAPHY_TEST_SOURCE}
        ${HP2_PATH_SOURCE}
    )
    # hp2_nativetextcore is an OBJECT library: its objects do not survive
    # transitively through hp2_xopengldrv, so consume them directly here.
    target_sources(hp2_native_typography_tests PRIVATE
        $<TARGET_OBJECTS:hp2_nativetextcore>
    )
    target_link_libraries(hp2_native_typography_tests PRIVATE
        hp2_core
        hp2_engine
        hp2_render
        hp2_xopengldrv
        ${HP2_TEXT_PROVIDER_LINK_LIBS}
    )
endif()

if(HP2_HAS_NATIVE_TEXT_BACKEND)
    hp2_add_executable(hp2_canvas_compatibility_tests
        ${HP2_CANVAS_COMPATIBILITY_TEST_SOURCE}
        ${HP2_PATH_SOURCE}
    )
    # Same object-library propagation rule as the typography tests above.
    target_sources(hp2_canvas_compatibility_tests PRIVATE
        $<TARGET_OBJECTS:hp2_nativetextcore>
    )
    target_link_libraries(hp2_canvas_compatibility_tests PRIVATE
        hp2_core
        hp2_engine
        hp2_render
        hp2_xopengldrv
        ${HP2_TEXT_PROVIDER_LINK_LIBS}
    )
endif()

set(HP2_EXECUTABLE_TARGETS
    hp2_game
    hp2_ucc
    hp2_package_audit
    hp2_abi_tests
    hp2_spell_runtime_tests
    hp2_eaxa_tests
    hp2_dxt1_tests
    hp2_audio_tests
    hp2_launcher_tests
    hp2_config_ini_tests
    hp2_replay_roundtrip_tests
    hp2_save_wire_oracle
    hp2_input_contract_tests
)

if(HP2_HAS_NATIVE_TEXT_BACKEND)
    list(APPEND HP2_EXECUTABLE_TARGETS hp2_native_typography_tests)
endif()

if(HP2_HAS_NATIVE_TEXT_BACKEND)
    list(APPEND HP2_EXECUTABLE_TARGETS hp2_canvas_compatibility_tests)
endif()

# Build every executable the behavioral test graph can invoke before CTest
# starts, so sanitizer/preset runs never register tests that point at missing
# binaries. Script-backed tests (Python) need no compiled target.
add_custom_target(hp2_verification_binaries
    DEPENDS ${HP2_EXECUTABLE_TARGETS}
    COMMENT "Building every executable referenced by the HP2 behavioral test graph"
)

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
    file(MAKE_DIRECTORY "${_hp2_test_home}" "${_hp2_test_tmp}" "${_hp2_test_artifacts}")

    add_test(NAME "${test_name}" COMMAND "${target}" ${ARGN})
    set(_hp2_test_env
        "HOME=${_hp2_test_home}"
        "TMPDIR=${_hp2_test_tmp}"
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
    set_tests_properties("${test_name}" PROPERTIES
        WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}"
        # Deterministic contracts must never hang a verification run; a
        # timeout is a first-class failure with retained artifacts.
        TIMEOUT 900
        ENVIRONMENT "${_hp2_test_env}"
    )
endfunction()

find_package(Python3 REQUIRED COMPONENTS Interpreter)

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

hp2_add_behavior_test(abi_widths hp2_abi_tests
    --test=abi_widths
)
hp2_add_behavior_test(render_clip hp2_abi_tests
    --test=render_clip
)
hp2_add_behavior_test(projection_fov hp2_abi_tests
    --test=projection_fov
)
hp2_add_behavior_test(command_line_load hp2_abi_tests
    --test=command_line_load
)
hp2_add_behavior_test(compact_index hp2_abi_tests
    --test=compact_index
)
hp2_add_behavior_test(fstring_archive hp2_abi_tests
    --test=fstring_archive
)
hp2_add_behavior_test(native_registration hp2_abi_tests
    --test=native_registration
    "-datadir=${HP2_TEST_DATA_ROOT}"
)
hp2_add_behavior_test(package79_manifest hp2_package_audit
    "-datadir=${HP2_TEST_DATA_ROOT}"
    "--reference=${CMAKE_BINARY_DIR}/goldens/package79-reference.json"
)
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
if(EXISTS "${PROJECT_SOURCE_DIR}/HarryPotter1/Unreal/System/Default.ini")
    add_custom_target(regenerate_goldens
        COMMAND "${CMAKE_COMMAND}" -E make_directory "${CMAKE_BINARY_DIR}/goldens"
        COMMAND "${Python3_EXECUTABLE}" "${PROJECT_SOURCE_DIR}/Build/package79_reference.py"
            "--data-root=${HP2_TEST_DATA_ROOT}"
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
            "--data-root=${HP2_TEST_DATA_ROOT}"
            "--output=${CMAKE_BINARY_DIR}/goldens/package79-reference.json"
        COMMAND "${Python3_EXECUTABLE}" "${PROJECT_SOURCE_DIR}/Build/spell_interaction_audit.py"
            "--repo-root=${PROJECT_SOURCE_DIR}"
            "--data-root=${HP2_TEST_DATA_ROOT}"
            "--output=${CMAKE_BINARY_DIR}/goldens/spell-interactions.json"
        WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}"
        COMMENT "Regenerating machine-local goldens from the configured test data root (no HP1 data root present)")
endif()
hp2_add_behavior_test(spell_runtime_contracts hp2_spell_runtime_tests
    "-datadir=${HP2_TEST_DATA_ROOT}"
)
hp2_add_behavior_test(dxt1_codec hp2_dxt1_tests)
hp2_add_behavior_test(eaxa_decoder hp2_eaxa_tests)
hp2_add_behavior_test(replay_roundtrip hp2_replay_roundtrip_tests)
hp2_add_behavior_test(config_ini_parse_semantics hp2_config_ini_tests
    --test=parse_semantics
)
hp2_add_behavior_test(config_ini_typed_getters hp2_config_ini_tests
    --test=typed_getters
)
hp2_add_behavior_test(config_ini_set_and_write hp2_config_ini_tests
    --test=set_and_write
)
hp2_add_behavior_test(config_ini_rewrite_normalization hp2_config_ini_tests
    --test=rewrite_normalization
)
hp2_add_behavior_test(config_ini_cache_filenames hp2_config_ini_tests
    --test=cache_filenames
)
hp2_add_behavior_test(config_ini_unicode_roundtrip hp2_config_ini_tests
    --test=unicode_roundtrip
)
# Audio lifecycle exercises a prototype music file that ships as untracked
# local reference data; without it the test cannot exist (configuration gap).
if(EXISTS "${HP2_UNREAL_ROOT}/Music/sm_bur_PlayfulFail_01.ogg")
    hp2_add_behavior_test(audio_lifecycle hp2_audio_tests)
    set_tests_properties(audio_lifecycle PROPERTIES
        DEPENDS "eaxa_decoder;native_registration"
        LABELS "fast;data-none"
    )
endif()
hp2_add_behavior_test(input_edge_contracts hp2_input_contract_tests
    --test=input_edge
    "-datadir=${HP2_TEST_DATA_ROOT}"
)
hp2_add_behavior_test(input_axis_order hp2_input_contract_tests
    --test=input_axis_order
    "-datadir=${HP2_TEST_DATA_ROOT}"
)
hp2_add_behavior_test(input_release_all hp2_input_contract_tests
    --test=input_release_all
    "-datadir=${HP2_TEST_DATA_ROOT}"
)
hp2_add_behavior_test(mouse_capture_policy hp2_input_contract_tests
    --test=mouse_capture_policy
    "-datadir=${HP2_TEST_DATA_ROOT}"
)
hp2_add_behavior_test(input_event_mapping hp2_input_contract_tests
    --test=input_event_mapping
    "-datadir=${HP2_TEST_DATA_ROOT}"
)
hp2_add_behavior_test(native_launcher_contract hp2_launcher_tests)
hp2_add_behavior_test(game_test_contract "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/GameTestTests.py"
)
hp2_add_behavior_test(repair_save_contract "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/RepairSaveTests.py"
)
hp2_add_behavior_test(ucc_smoke_contract "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/UccSmokeTests.py"
)
if(EXISTS "${CMAKE_BINARY_DIR}/hp2_ucc")
    hp2_add_behavior_test(ucc_help_smoke "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/UccSmokeTests.py"
        --smoke "--ucc-binary=${CMAKE_BINARY_DIR}/hp2_ucc"
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

    # HP1 class bind: Engine.Music resolves through the native registry
    # (IMPLEMENT_CLASS(UMusic) + the VerifyImport RF_Native fallback), and a
    # live hp2_ucc run verifies Engine.u/Editor.u under the HP1 root with zero
    # unresolved imports. Same data-root gate as hp1_loader_manifest; the
    # runtime layer needs the already-built ucc binary.
    hp2_add_behavior_test(hp1_class_bind "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/HP1ClassBindTests.py" -v
    )
    set_tests_properties(hp1_class_bind PROPERTIES
        LABELS "integration;data-hp1"
        ENVIRONMENT "HP2_UCC_BINARY=${CMAKE_BINARY_DIR}/hp2_ucc"
    )
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
endif()
hp2_add_behavior_test(framing_baseline_contract "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/FramingBaselineTests.py"
)
set_tests_properties(framing_baseline_contract PROPERTIES LABELS "fast;data-none")
hp2_add_behavior_test(matrix_compare_contract "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/MatrixCompareTests.py"
)
set_tests_properties(matrix_compare_contract PROPERTIES LABELS "fast;data-none")

if(EXISTS "${PROJECT_SOURCE_DIR}/dist/macos-arm64/HarryPotter2.app"
        AND EXISTS "${HP2_TEST_DATA_ROOT}/System/Default.ini")
    hp2_add_behavior_test(determinism_double_run "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/DeterminismDoubleRunTests.py"
        --ticks=60
    )
    set_tests_properties(determinism_double_run PROPERTIES
        LABELS "integration;data-prototype"
        RESOURCE_LOCK hp2_gpu
    )
endif()


if(HP2_HAS_NATIVE_TEXT_BACKEND)
    # Data-backed contracts pass their root explicitly (never CWD discovery),
    # so a local out/retail-data import can never hijack the fixture path.
    hp2_add_behavior_test(native_typography_contracts hp2_native_typography_tests
        "-datadir=${HP2_TEST_DATA_ROOT}"
    )
endif()

if(HP2_HAS_NATIVE_TEXT_BACKEND)
    # The immutability canaries hash three prototype code packages that ship
    # as untracked local reference data; gate exactly like the data itself.
    if(EXISTS "${HP2_UNREAL_ROOT}/System/Engine.u"
            AND EXISTS "${HP2_UNREAL_ROOT}/System/HGame.u"
            AND EXISTS "${HP2_UNREAL_ROOT}/System/UWindow.u")
        hp2_add_behavior_test(canvas_compatibility_contracts hp2_canvas_compatibility_tests
            "-datadir=${HP2_TEST_DATA_ROOT}"
        )
        set_tests_properties(canvas_compatibility_contracts
            PROPERTIES LABELS "integration;data-prototype"
        )
    endif()
endif()

# Deterministic renderer smoke tiers. The commit gate runs a fixed three-map
# subset; the full sweep is registered under the smoke-full label and
# excluded from default test presets because it launches every playable map.
# Both require the installed app bundle and prototype data at configure time;
# without them the tests are not silently skipped, they simply do not exist,
# which the verification docs call out as a configuration gap.
if(EXISTS "${PROJECT_SOURCE_DIR}/dist/macos-arm64/HarryPotter2.app"
        AND EXISTS "${HP2_TEST_DATA_ROOT}/System/Default.ini")
    hp2_add_behavior_test(renderer_smoke_xopengl "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Build/smoke_maps.py"
        "--renderer=xopengl"
        "--app=${CMAKE_BINARY_DIR}/hp2_game"
        "--maps=PrivetDr,Entry,Ch2Skurge"
        "--ticks=120"
        "--timeout=90"
        "--output=${CMAKE_BINARY_DIR}/Testing/HP2/renderer_smoke_xopengl/smoke-maps.json"
        "--data-root=${HP2_TEST_DATA_ROOT}"
    )
    if(HP2_ENABLE_FULL_MAP_SMOKE)
        hp2_add_behavior_test(renderer_smoke_full "${Python3_EXECUTABLE}"
            "${PROJECT_SOURCE_DIR}/Build/smoke_maps.py"
            "--app=${CMAKE_BINARY_DIR}/hp2_game"
            "--ticks=300"
            "--output=${CMAKE_BINARY_DIR}/Testing/HP2/renderer_smoke_full/smoke-maps.json"
            "--data-root=${HP2_TEST_DATA_ROOT}"
        )
        set_tests_properties(renderer_smoke_full PROPERTIES
            LABELS "smoke;renderer;xopengl;smoke-full"
            RESOURCE_LOCK hp2_gpu
            TIMEOUT 14400
        )
    endif()
endif()

# ABI and registration checks are prerequisites for package/runtime tests.
# Codec tests are otherwise independent; audio lifecycle additionally requires
# the decoder and native registration contracts.
set_tests_properties(compact_index fstring_archive native_registration
    PROPERTIES DEPENDS abi_widths
)
set_tests_properties(package79_manifest PROPERTIES
    DEPENDS "compact_index;fstring_archive;native_registration"
)
set_tests_properties(spell_runtime_contracts PROPERTIES
    DEPENDS "native_registration;package79_manifest;spell_interaction_manifest"
)
set_tests_properties(native_launcher_contract game_test_contract repair_save_contract
    ucc_smoke_contract prototype_archive_contract
    PROPERTIES LABELS "fast;data-none"
)
set_tests_properties(dxt1_codec eaxa_decoder PROPERTIES DEPENDS abi_widths)

# Label taxonomy: layer labels (fast|integration|smoke|visual|manual) plus a
# data-profile label (data-none|data-prototype|data-retail). Smoke tests keep
# the labels assigned in the smoke block above.
set_tests_properties(abi_widths render_clip projection_fov command_line_load
    compact_index fstring_archive dxt1_codec eaxa_decoder
    native_launcher_contract game_test_contract repair_save_contract
    PROPERTIES LABELS "fast;data-none"
)
set_tests_properties(native_registration package79_manifest
    spell_interaction_manifest spell_runtime_contracts
    PROPERTIES LABELS "integration;data-prototype"
)
if(HP2_HAS_NATIVE_TEXT_BACKEND)
    set_tests_properties(native_typography_contracts
        PROPERTIES LABELS "fast;data-none"
    )
endif()
# ---------------------------------------------------------------------------
# Experimental Vulkan render device: HP2 port of ThirdParty/UT99VulkanDrv
# (ZVulkan-lineage community UE1 driver). Opt-in only; stays OFF by default
# until first successful launch evidence, exactly like the -vulkan feature
# state. Everything below this comment is owned by the Vulkan port block.
#
# Layout mirrors hp2_xopengldrv: object libraries linked into hp2_game so the
# engine's static class registry finds VulkanDrv.VulkanRenderDevice at boot.
# ---------------------------------------------------------------------------
option(HP2_ENABLE_VULKAN_DRIVER
    "Build the experimental Vulkan render device port (ThirdParty/UT99VulkanDrv)" OFF)

if(HP2_ENABLE_VULKAN_DRIVER)
    set(HP2_VULKANDRV_ROOT "${HP2_THIRD_PARTY_ROOT}/UT99VulkanDrv")

    # ZVulkan core + vendored volk / vk_mem_alloc / glslang (runtime GLSL ->
    # SPIR-V for ShaderManager). Third-party code is exempt from the repo
    # warning policy; only hp2_vulkandrv itself compiles under it.
    set(HP2_ZVULKAN_SOURCES
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/vulkanbuilders.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/vulkandevice.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/vulkaninstance.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/vulkansurface.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/vulkanswapchain.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/vk_mem_alloc/vk_mem_alloc.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/volk/volk.c"
        # vendored glslang (Unix OSDependent layer)
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/GenericCodeGen/CodeGen.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/GenericCodeGen/Link.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/Constant.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/InfoSink.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/Initialize.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/IntermTraverse.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/Intermediate.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/ParseContextBase.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/ParseHelper.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/PoolAlloc.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/RemoveTree.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/Scan.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/ShaderLang.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/SpirvIntrinsics.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/SymbolTable.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/Versions.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/attribute.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/glslang_tab.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/intermOut.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/iomapper.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/limits.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/linkValidate.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/parseConst.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/propagateNoContraction.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/reflection.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/preprocessor/Pp.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/preprocessor/PpAtom.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/preprocessor/PpContext.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/preprocessor/PpScanner.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/MachineIndependent/preprocessor/PpTokens.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/OSDependent/Unix/ossource.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/glslang/ResourceLimits/ResourceLimits.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/spirv/GlslangToSpv.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/spirv/InReadableOrder.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/spirv/Logger.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/spirv/SpvBuilder.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/spirv/SpvPostProcess.cpp"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src/glslang/spirv/SpvTools.cpp"
    )

    add_library(hp2_zvulkan OBJECT ${HP2_ZVULKAN_SOURCES})
    target_compile_features(hp2_zvulkan PUBLIC cxx_std_17)
    target_include_directories(hp2_zvulkan PUBLIC
        "${HP2_VULKANDRV_ROOT}/ZVulkan/include"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/include/zvulkan"
        "${HP2_VULKANDRV_ROOT}/ZVulkan/src"
    )
    # The donor's ZVulkan CMake defines UNIX/_UNIX on non-Win32 for its
    # glslang OSDependent selection and volk platform branches.
    target_compile_definitions(hp2_zvulkan PRIVATE UNIX=1 _UNIX=1)

    set(HP2_VULKANDRV_SOURCES
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/UVulkanRenderDevice.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/VulkanDrv.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/BufferManager.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/CommandBufferManager.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/DescriptorSetManager.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/FileResource.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/FramebufferManager.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/Hp2FrameCapture.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/halffloat.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/mat.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/RenderPassManager.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/SamplerManager.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/SceneTextures.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/ShaderManager.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/TextureManager.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/TextureUploader.cpp"
        "${HP2_VULKANDRV_ROOT}/VulkanDrv/UploadManager.cpp"
    )

    add_library(hp2_vulkandrv OBJECT ${HP2_VULKANDRV_SOURCES})
    target_link_libraries(hp2_vulkandrv PRIVATE hp2_compile_policy)
    target_link_libraries(hp2_vulkandrv PUBLIC hp2_zvulkan)
    target_include_directories(hp2_vulkandrv PUBLIC
        ${HP2_RENDER_INCLUDE_DIRS}
        "${HP2_VULKANDRV_ROOT}/VulkanDrv"
    )
    target_compile_definitions(hp2_vulkandrv PRIVATE
        __STATIC_LINK=1
        SDL2BUILD=1
    )
    target_link_libraries(hp2_vulkandrv PUBLIC SDL2::SDL2-static)

    if(HP2_HAS_NATIVE_TEXT_BACKEND)
        # The experimental Vulkan driver shares the driver-neutral CoreText
        # core with XOpenGL: inject its objects, the shared header include
        # path, and the Vulkan-side native text translation unit authored
        # against Inc/NativeTextShared.h.
        target_sources(hp2_vulkandrv PRIVATE
            "${HP2_VULKANDRV_ROOT}/VulkanDrv/VulkanNativeText.cpp"
        )
        target_include_directories(hp2_vulkandrv PUBLIC "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Inc")
        target_link_libraries(hp2_vulkandrv PUBLIC hp2_nativetextcore)
        # Objects do not flow through the object-library chain: register the
        # shared native-text core next to hp2_vulkandrv, mirroring hp2_zvulkan.
        target_link_libraries(hp2_game PRIVATE hp2_nativetextcore)
    endif()

    # Static registration: adding the objects to hp2_game puts
    # VulkanDrv.VulkanRenderDevice into the engine class registry next to
    # XOpenGLDrv.XOpenGLRenderDevice; config/-flag selection stays lead-owned.
    # The define gates RegisterHP2ClientClasses' registrant call.
    target_link_libraries(hp2_game PRIVATE
        hp2_vulkandrv
        hp2_zvulkan
    )
    target_compile_definitions(hp2_game PRIVATE HP2_ENABLE_VULKAN_DRIVER=1)

    find_package(Python3 REQUIRED COMPONENTS Interpreter)
    add_test(NAME renderer_smoke_vulkan
        COMMAND "${Python3_EXECUTABLE}"
            "${PROJECT_SOURCE_DIR}/Build/run_vulkan_smoke.py"
            "--build-dir=${CMAKE_BINARY_DIR}"
            "--data-root=${HP2_TEST_DATA_ROOT}"
        WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}"
    )
    set_tests_properties(renderer_smoke_vulkan PROPERTIES
        LABELS "smoke;renderer;vulkan"
        RESOURCE_LOCK hp2_gpu
        TIMEOUT 600
    )

endif()
