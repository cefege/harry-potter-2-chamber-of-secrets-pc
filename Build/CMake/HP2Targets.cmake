add_library(hp2_compile_policy INTERFACE)
target_compile_features(hp2_compile_policy INTERFACE cxx_std_17)

target_compile_definitions(hp2_compile_policy INTERFACE
    __UNIX__=1
    MACOSX=1
    ASM=0
    UNICODE=1
    _UNICODE=1
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
    "${HP2_THIRD_PARTY_ROOT}/UT469eSDK/SDLDrv/Inc"
    "${HP2_THIRD_PARTY_ROOT}/UT469eSDK/SDLDrv/Src"
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
    ${HP2_LAUNCHER_TEST_SOURCE}
)
target_compile_definitions(hp2_launcher_tests PRIVATE HP2_LAUNCHER_TESTING=1)

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
)

# Give every behavioral test an isolated, deterministic HOME. HP2 derives its
# writable Application Support tree from HOME; immutable package data is
# always selected explicitly with -datadir when runtime bootstrap is needed.
function(hp2_add_behavior_test test_name target)
    set(_hp2_test_root "${CMAKE_BINARY_DIR}/Testing/HP2/${test_name}")
    set(_hp2_test_home "${_hp2_test_root}/Home")
    set(_hp2_test_tmp "${_hp2_test_root}/Tmp")
    file(MAKE_DIRECTORY "${_hp2_test_home}" "${_hp2_test_tmp}")

    add_test(NAME "${test_name}" COMMAND "${target}" ${ARGN})
    set_tests_properties("${test_name}" PROPERTIES
        WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}"
        ENVIRONMENT
            "HOME=${_hp2_test_home};TMPDIR=${_hp2_test_tmp};LC_ALL=C;TZ=UTC"
    )
endfunction()

find_package(Python3 REQUIRED COMPONENTS Interpreter)

hp2_add_behavior_test(abi_widths hp2_abi_tests
    --test=abi_widths
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
    "-datadir=${HP2_UNREAL_ROOT}"
)
hp2_add_behavior_test(package79_manifest hp2_package_audit
    "-datadir=${HP2_UNREAL_ROOT}"
    "--reference=${PROJECT_SOURCE_DIR}/Tests/Fixtures/package79-reference.json"
)
hp2_add_behavior_test(spell_interaction_manifest "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Build/spell_interaction_audit.py"
    "--repo-root=${PROJECT_SOURCE_DIR}"
    "--data-root=${HP2_UNREAL_ROOT}"
    "--output=${PROJECT_SOURCE_DIR}/Tests/Fixtures/spell-interactions.json"
    --check
)
set_tests_properties(spell_interaction_manifest PROPERTIES
    WORKING_DIRECTORY "${CMAKE_BINARY_DIR}/Testing/HP2/spell_interaction_manifest"
)
hp2_add_behavior_test(spell_runtime_contracts hp2_spell_runtime_tests
    "-datadir=${HP2_UNREAL_ROOT}"
)
hp2_add_behavior_test(dxt1_codec hp2_dxt1_tests)
hp2_add_behavior_test(eaxa_decoder hp2_eaxa_tests)
hp2_add_behavior_test(audio_lifecycle hp2_audio_tests)
hp2_add_behavior_test(native_launcher_contract hp2_launcher_tests)

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
set_tests_properties(dxt1_codec eaxa_decoder PROPERTIES DEPENDS abi_widths)
set_tests_properties(audio_lifecycle PROPERTIES
    DEPENDS "eaxa_decoder;native_registration"
)
