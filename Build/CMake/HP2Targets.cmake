# Root of the immutable prototype data tree handed to behavioral tests via
# -datadir/--data-root. Defaults to the build's prototype checkout so test
# runs can be pointed at an alternate data root without editing this file.
set(HP2_TEST_DATA_ROOT "${HP2_UNREAL_ROOT}" CACHE PATH
    "Prototype data root passed to behavioral tests via -datadir/--data-root")

add_library(hp2_compile_policy INTERFACE)
target_compile_features(hp2_compile_policy INTERFACE cxx_std_17)

target_compile_definitions(hp2_compile_policy INTERFACE
    ASM=0
    UNICODE=1
    _UNICODE=1
    HP2_HAS_NATIVE_TEXT_BACKEND=$<BOOL:${HP2_HAS_NATIVE_TEXT_BACKEND}>
)
if(APPLE)
    target_compile_definitions(hp2_compile_policy INTERFACE MACOSX=1 __UNIX__=1)
elseif(WIN32)
    target_compile_definitions(hp2_compile_policy INTERFACE
        HP2_WINDOWS=1
        WIN32_LEAN_AND_MEAN
        NOMINMAX
        _WIN32_WINNT=0x0A00
    )
else()
    target_compile_definitions(hp2_compile_policy INTERFACE __UNIX__=1)
endif()

if(CMAKE_CXX_COMPILER_ID MATCHES "^(AppleClang|Clang)$")
    target_compile_options(hp2_compile_policy INTERFACE
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-fsigned-char>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wall>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wextra>"
        # The VC6-era headers intentionally expose empty virtual defaults,
        # placement-new forms, and null-base reflected offset expressions.
        # Keep host-width conversions fatal while silencing that known noise.
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wno-unused-parameter>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wno-reorder-ctor>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wno-inline-new-delete>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wno-implicit-exception-spec-mismatch>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wno-deprecated-copy>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wno-exceptions>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wno-ignored-qualifiers>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wno-sign-compare>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wno-overloaded-virtual>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wno-extra-tokens>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wno-string-plus-int>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Wno-null-pointer-subtraction>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Werror=shorten-64-to-32>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Werror=pointer-to-int-cast>"
        "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-Werror=int-to-pointer-cast>"
        "$<$<AND:$<CONFIG:Release>,$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>>:-O3>"
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
    WGPUDRV_API=
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
set(HP2_NATIVETEXT_INCLUDE_DIRS
    "${HP2_UNREAL_ROOT}/NativeText/Inc"
)
set(HP2_LAUNCH_INCLUDE_DIRS
    ${HP2_RENDER_INCLUDE_DIRS}
    ${HP2_FIRE_INCLUDE_DIRS}
    ${HP2_ALAUDIO_INCLUDE_DIRS}
    ${HP2_SDLDRV_INCLUDE_DIRS}
    ${HP2_NATIVETEXT_INCLUDE_DIRS}
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

# The Rust workspace builds one staticlib, hp2-ffi, which re-exports every
# port crate's extern "C" API. Its build.rs writes the cbindgen header
# hp2_rs.h into HP2_RS_INCLUDE_DIR. Linking it through hp2_core reaches
# hp2_game, hp2_ucc, and every executable that links the Core object library.
#
# hp2_rs.h is declared a BYPRODUCT of corrosion's `_cargo-build_hp2_ffi`
# utility target (below, after HP2_RS_INCLUDE_DIR is set). Without that,
# nothing told Ninja that this header is generated by the cargo step, so
# a C++ consumer's freshness was judged against the header's mtime from
# the start of the invocation: a single `cmake --build` that changed an
# hp2_rs_* signature relinked already-built consumers against the new
# libhp2_ffi.a without recompiling them against the regenerated header (a
# silent extern "C" ABI mismatch), and only the next invocation rebuilt
# them (observed: ~37 consumers recompiled one invocation late). As a
# byproduct, the header gets Ninja's restat after the cargo step and every
# consumer whose depfile lists it is ordered after, and rebuilt by, the same
# invocation. cbindgen only rewrites the file when its contents change, so
# an unchanged API rebuilds nothing.
# Verified (wgpu tree, header-only signature change, one consumer): with the
# BYPRODUCTS below removed, the single invocation rewrote hp2_rs.h and exited 0
# without recompiling WgpuDrv.cpp.o (stale). With it present, the same
# invocation rewrote the header and recompiled WgpuDrv.cpp.o, which then failed
# against the new declaration. One build is enough; the run-twice step is not
# needed for this case. Only one consumer was tested, not all ~37.
#
# Corrosion copies CMAKE_OSX_DEPLOYMENT_TARGET into MACOSX_DEPLOYMENT_TARGET for
# every cargo run, host proc-macros included. On macOS 27 with Xcode ld-27037,
# the host dylib for cbindgen's serde_derive then fails dyld's LINKEDIT check
# ("mis-aligned LINKEDIT string pool") whenever that variable is set. hp2-ffi
# has no C sources, so the forwarding does nothing useful for it. Clear it for
# this import only; the game's own targets keep CMAKE_OSX_DEPLOYMENT_TARGET.
# Emscripten has no Rust target, so the browser build links no Rust.
#
# WgpuDrv is the only render device, built on every non-web platform. The web
# build links no Rust, so it has no render device there.
if(NOT EMSCRIPTEN)
    set(_hp2_saved_osx_deployment_target "${CMAKE_OSX_DEPLOYMENT_TARGET}")
    set(CMAKE_OSX_DEPLOYMENT_TARGET "")
    corrosion_import_crate(MANIFEST_PATH "${PROJECT_SOURCE_DIR}/Cargo.toml" CRATES hp2-ffi)
    # hp2-import's `archive` Cargo feature links the real pinned
    # archive_static for its Extract/HP2ImportArchive.cpp replacement
    # (crates/hp2-import/src/archive.rs) -- off by default so bare
    # `cargo build`/`cargo test` never need libarchive, enabled here for
    # the real product build where it's actually needed. See
    # crates/hp2-import/Cargo.toml's `[features]` doc comment.
    set(HP2_FFI_FEATURES hp2-import/archive platform-sdl)
    # renderer-wgpu: the WgpuDrv.WgpuRenderDevice backend. Off by default in
    # crates/hp2-ffi so bare cargo build/test never needs wgpu's platform backends
    # (objc2-metal and friends); always on for the real product build.
    list(APPEND HP2_FFI_FEATURES renderer-wgpu)
    corrosion_set_features(hp2_ffi FEATURES ${HP2_FFI_FEATURES})
    # libhp2_ffi.a's compiled code (not just the Import-specific part)
    # has real, unresolved archive_read_*/archive_entry_*/archive_error_*
    # symbol references now that the feature above is enabled -- the
    # Cargo feature is a property of the whole crate, not something
    # selectable per consumer, so every target linking hp2_ffi (hp2_core
    # included, transitively reaching nearly every executable in this
    # project) needs archive_static resolvable at its own final link,
    # not just hp2_import_core's consumers. Linked on hp2_ffi's own
    # INTERFACE so it propagates everywhere hp2_ffi already does.
    #
    # Same reasoning for SDL2::SDL2-static, enabled by the platform-sdl
    # feature above (crates/hp2-platform/src/clipboard.rs's doc comment
    # has the full design and why it is hand-written extern "C"
    # declarations rather than an `sdl2`/`sdl2-sys` crate dependency --
    # deliberately so, to avoid the staticlib-bundling duplicate-SDL2
    # risk a crate-managed static link would have). SDL2::SDL2-static is
    # the exact same FetchContent-built archive every other SDL2
    # consumer in this project already links (SDLDrv, SDLLaunch,
    # HP2LauncherPlatform.cpp) -- this adds no second copy, just one
    # more consumer of the one archive, resolved once at hp2_game's
    # final link.
    target_link_libraries(hp2_ffi INTERFACE archive_static SDL2::SDL2-static)
    if(APPLE)
        # Not guessed: `cargo rustc -p hp2-ffi --lib --crate-type=staticlib
        # --features=hp2-import/archive,platform-sdl,renderer-wgpu --
        # --print native-static-libs` printed the authoritative list
        # libhp2_ffi.a actually needs once renderer-wgpu (wgpu's Metal
        # backend: objc2-metal, raw-window-metal, ...) is enabled:
        # QuartzCore, CoreGraphics, Metal, Foundation, CoreFoundation,
        # objc, iconv (System/c/m already resolve implicitly). The link
        # succeeded even before this list was added -- AppKit, already
        # linked unconditionally into hp2_game, pulls some of these in
        # transitively -- but relying on that would be fragile; this is
        # the real, verified list, not a guess. Linux's wgpu
        # backends are not covered here -- wgpu's non-Apple hals load
        # their native libraries dynamically rather than needing
        # link-time framework linking, unverified in this project's own
        # build yet.
        target_link_libraries(hp2_ffi INTERFACE
            "-framework QuartzCore" "-framework CoreGraphics" "-framework Metal"
            "-framework Foundation" "-framework CoreFoundation" objc iconv)
    endif()
    set(CMAKE_OSX_DEPLOYMENT_TARGET "${_hp2_saved_osx_deployment_target}")
    set(HP2_RS_INCLUDE_DIR "${CMAKE_BINARY_DIR}/rust-include")
    file(MAKE_DIRECTORY "${HP2_RS_INCLUDE_DIR}")
    corrosion_set_env_vars(hp2_ffi "HP2_RS_INCLUDE_DIR=${HP2_RS_INCLUDE_DIR}")
    target_include_directories(hp2_ffi INTERFACE "${HP2_RS_INCLUDE_DIR}")
    # See the hp2_rs.h note above this block. `_cargo-build_<target>` is
    # corrosion's utility target that runs cargo; its build-event command
    # joins the existing restat'd byproduct-copy step.
    add_custom_command(TARGET _cargo-build_hp2_ffi POST_BUILD
        COMMAND "${CMAKE_COMMAND}" -E true
        BYPRODUCTS "${HP2_RS_INCLUDE_DIR}/hp2_rs.h"
        VERBATIM)
endif()

hp2_add_engine_object(hp2_core Core ${HP2_CORE_SOURCES})
target_include_directories(hp2_core PUBLIC ${HP2_CORE_INCLUDE_DIRS})
if(NOT EMSCRIPTEN)
    target_link_libraries(hp2_core PUBLIC hp2_ffi)
endif()
if(WIN32)
    target_link_libraries(hp2_core PUBLIC SDL2::SDL2-static ole32 shell32 shlwapi ws2_32)
endif()

hp2_add_engine_object(hp2_engine Engine ${HP2_ENGINE_SOURCES})
target_include_directories(hp2_engine PUBLIC ${HP2_ENGINE_INCLUDE_DIRS})
target_include_directories(hp2_engine PRIVATE ${HP2_NATIVETEXT_INCLUDE_DIRS})
target_link_libraries(hp2_engine PUBLIC Squish::Squish)
if(NOT EMSCRIPTEN)
    target_link_libraries(hp2_engine PRIVATE hp2_ffi)
endif()

hp2_add_engine_object(hp2_render Render ${HP2_RENDER_SOURCES})
target_include_directories(hp2_render PUBLIC ${HP2_RENDER_INCLUDE_DIRS})
target_link_libraries(hp2_render PUBLIC Squish::Squish)
if(NOT EMSCRIPTEN)
    target_link_libraries(hp2_render PRIVATE hp2_ffi)
endif()

hp2_add_engine_object(hp2_fire Fire ${HP2_FIRE_SOURCES})
target_include_directories(hp2_fire PUBLIC ${HP2_FIRE_INCLUDE_DIRS})
if(NOT EMSCRIPTEN)
    # UnFractal.cpp calls hp2_rs_fire_* and includes hp2_rs.h; Core.h does not
    # carry that include, so the Fire object needs hp2_ffi's include dir here.
    target_link_libraries(hp2_fire PRIVATE hp2_ffi)
endif()

hp2_add_engine_object(hp2_editor_runtime Editor ${HP2_EDITOR_RUNTIME_SOURCES})
target_include_directories(hp2_editor_runtime PRIVATE ${HP2_EDITOR_INCLUDE_DIRS})

hp2_add_engine_object(hp2_alaudio ALAudio ${HP2_ALAUDIO_SOURCES})
target_include_directories(hp2_alaudio PUBLIC ${HP2_ALAUDIO_INCLUDE_DIRS})
target_link_libraries(hp2_alaudio PUBLIC OpenAL::OpenAL)
if(NOT EMSCRIPTEN)
    target_link_libraries(hp2_alaudio PRIVATE hp2_ffi)
endif()

hp2_add_engine_object(hp2_sdldrv SDLDrv ${HP2_SDLDRV_SOURCES})
target_include_directories(hp2_sdldrv PUBLIC ${HP2_SDLDRV_INCLUDE_DIRS})
target_link_libraries(hp2_sdldrv PUBLIC SDL2::SDL2-static)
if(NOT EMSCRIPTEN)
    # SDLClient.cpp calls hp2_rs_platform_get_clipboard_text/
    # set_clipboard_text (crates/hp2-platform/src/clipboard.rs) directly;
    # needs hp2_ffi's generated hp2_rs.h on its own account, same as
    # hp2_core's own `target_link_libraries(hp2_core PUBLIC hp2_ffi)`
    # above -- hp2_sdldrv is a separate object library that does not
    # itself link hp2_core, so it does not inherit that propagation.
    target_link_libraries(hp2_sdldrv PUBLIC hp2_ffi)
endif()

set(HP2_IMGUI_ROOT "${hp2_imgui_SOURCE_DIR}")
add_library(hp2_imgui STATIC
    "${HP2_IMGUI_ROOT}/imgui.cpp"
    "${HP2_IMGUI_ROOT}/imgui_draw.cpp"
    "${HP2_IMGUI_ROOT}/imgui_tables.cpp"
    "${HP2_IMGUI_ROOT}/imgui_widgets.cpp"
    "${HP2_IMGUI_ROOT}/backends/imgui_impl_sdl2.cpp"
    "${HP2_IMGUI_ROOT}/backends/imgui_impl_sdlrenderer2.cpp"
)
target_include_directories(hp2_imgui PUBLIC "${HP2_IMGUI_ROOT}" "${HP2_IMGUI_ROOT}/backends")
target_compile_features(hp2_imgui PUBLIC cxx_std_17)
target_link_libraries(hp2_imgui PUBLIC SDL2::SDL2-static PRIVATE hp2_compile_policy)

if(HP2_HAS_NATIVE_TEXT_BACKEND)
    # Selected shaping/rasterization core (NativeTextShared.cpp for CoreText,
    # NativeTextFreeType.cpp for FreeType) compiled once and consumed as
    # objects by every render driver with a native text leg, so the provider
    # link libraries reach both consumers.
    hp2_add_engine_object(hp2_nativetextcore NativeText
        ${HP2_TEXT_PROVIDER_SOURCES}
        "${HP2_UNREAL_ROOT}/NativeText/Src/NativeText.cpp"
    )
    target_include_directories(hp2_nativetextcore PUBLIC ${HP2_NATIVETEXT_INCLUDE_DIRS} ${HP2_ENGINE_INCLUDE_DIRS})
    if(WIN32 OR EMSCRIPTEN)
        target_compile_definitions(hp2_nativetextcore PRIVATE HP2_NATIVE_TEXT_FONT_SEAM=1)
    endif()
    if(WIN32)
        target_link_libraries(hp2_nativetextcore PUBLIC SDL2::SDL2-static)
    endif()
    # Emscripten only builds the FreeType/HarfBuzz ports when asked, and the
    # headers are part of that, so the compile side needs the flag as well as
    # the link side. Without it <ft2build.h> is simply absent.
    if(EMSCRIPTEN)
        target_compile_options(hp2_nativetextcore PRIVATE -sUSE_FREETYPE=1 -sUSE_HARFBUZZ=1)
    endif()
    target_link_libraries(hp2_nativetextcore PUBLIC ${HP2_TEXT_PROVIDER_LINK_LIBS})
    # Emscripten's FreeType/HarfBuzz are emcc ports, not CMake libraries, so
    # they are requested as link flags on the final executable instead. Read
    # here (a global property, since the provider declares them) and applied
    # to hp2_game further down, where that target exists.
    get_property(HP2_WEB_FONTLINK_FLAGS GLOBAL PROPERTY HP2_WEB_FONTLINK_FLAGS)

endif()

set(HP2_IMPORT_ROOT "${HP2_UNREAL_ROOT}/Import")
if(EMSCRIPTEN)
    # The web build has no Rust at all: HP2Dependencies.cmake excludes
    # both Corrosion and libarchive entirely under EMSCRIPTEN, and
    # stable Rust has no wasm64-unknown-emscripten target to begin with
    # (confirmed directly: `rustc --print target-list` has no such
    # entry; the one wasm64 target that exists,
    # wasm64-unknown-unknown, has no prebuilt std on this pinned stable
    # toolchain -- `rustup target add` reports "no prebuilt artifacts",
    # requiring nightly + `-Z build-std` -- and still lacks Emscripten's
    # runtime glue regardless of that). The C++ importer stays for this
    # one platform until Rust gains a stable wasm64 Emscripten target,
    # or the project reconsiders wasm64 (`-sMEMORY64=1`, see the
    # web-wasm64 preset) vs wasm32 for the web build; this is a verified
    # upstream limitation, not a scope choice -- native (macOS/Linux)
    # fully replaces this with crates/hp2-import via hp2-ffi, below.
    add_library(hp2_import_core STATIC
        "${HP2_IMPORT_ROOT}/Src/HP2ImportInputs.cpp"
        "${HP2_IMPORT_ROOT}/Src/HP2ImportCollect.cpp"
        "${HP2_IMPORT_ROOT}/Src/HP2ImportPlan.cpp"
        "${HP2_IMPORT_ROOT}/Src/HP2ImportManifest.cpp"
        "${HP2_IMPORT_ROOT}/Src/HP2ImportOutput.cpp"
    )
    target_include_directories(hp2_import_core PUBLIC "${HP2_IMPORT_ROOT}/Inc"
        PRIVATE "${HP2_IMPORT_ROOT}/Src" "${HP2_UNREAL_ROOT}/Core/Inc")
    target_compile_features(hp2_import_core PUBLIC cxx_std_17)
    target_link_libraries(hp2_import_core PUBLIC hp2_compile_policy PRIVATE utf8proc::utf8proc)
else()
    # Native builds: crates/hp2-import (via hp2-ffi's hp2_rs_import_run)
    # fully replaces HP2Import{Inputs,Collect,Plan,Manifest,Output,
    # Archive}.cpp -- none of them compile here. hp2_import_core becomes
    # an INTERFACE library instead of a static one so every existing
    # consumer (hp2_import, hp2_game, hp2_launcher_tests) keeps its
    # unchanged `target_link_libraries(... hp2_import_core ...)` line
    # and transitively gets hp2_ffi (the real implementation, including
    # the real archive_static-backed Extract replacement -- see the
    # corrosion_import_crate block above for where hp2_ffi itself picks
    # up archive_static on its own INTERFACE, propagating to every
    # consumer including this one) plus HP2Import.h's public include
    # dir (Options/Progress/Profile/LinkMode and the RunImport
    # declaration -- still the real C++ surface HP2DataImport.cpp/
    # HP2ImportMain.cpp construct and call across the FFI boundary
    # with).
    add_library(hp2_import_core INTERFACE)
    target_include_directories(hp2_import_core INTERFACE "${HP2_IMPORT_ROOT}/Inc")
    target_link_libraries(hp2_import_core INTERFACE hp2_compile_policy hp2_ffi)
    if(WIN32)
        target_link_libraries(hp2_import_core INTERFACE bcrypt)
    endif()
    add_executable(hp2_import "${HP2_IMPORT_ROOT}/Src/HP2ImportMain.cpp")
    target_link_libraries(hp2_import PRIVATE hp2_import_core hp2_compile_policy hp2_ffi)
endif()

function(hp2_add_executable target)
    add_executable("${target}" ${ARGN})
    target_include_directories("${target}" PRIVATE ${HP2_LAUNCH_INCLUDE_DIRS})
    target_link_libraries("${target}" PRIVATE hp2_compile_policy)
    if(WIN32)
        target_link_options("${target}" PRIVATE -mconsole)
    endif()
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
if(WIN32)
    set(HP2_WINDOWS_ICON_PATH "${HP2_UNREAL_ROOT}/SDLLaunch/Res/Game.ico")
    set(HP2_WINDOWS_MANIFEST_PATH "${HP2_UNREAL_ROOT}/SDLLaunch/Res/HarryPotter2.manifest")
    set(HP2_WINDOWS_VERSION_RESOURCE "${CMAKE_CURRENT_BINARY_DIR}/HarryPotter2.rc")
    configure_file(
        "${HP2_UNREAL_ROOT}/SDLLaunch/Res/HarryPotter2.rc.in"
        "${HP2_WINDOWS_VERSION_RESOURCE}"
        @ONLY
    )
    target_sources(hp2_game PRIVATE "${HP2_WINDOWS_VERSION_RESOURCE}")
    set_target_properties(hp2_game PROPERTIES
        WIN32_EXECUTABLE TRUE
        OUTPUT_NAME HarryPotter2
    )
    target_link_libraries(hp2_game PRIVATE
        SDL2::SDL2main
        ole32 shell32 shlwapi ws2_32 winmm imm32 version setupapi
    )
endif()
if(APPLE)
    set_source_files_properties("${HP2_LAUNCHER_PLATFORM_MAC_SOURCE}" PROPERTIES
        COMPILE_OPTIONS "-fobjc-arc;-std=c++17"
    )
endif()
target_link_libraries(hp2_game PRIVATE
    hp2_core
    hp2_engine
    hp2_render
    hp2_fire
    hp2_editor_runtime
    hp2_alaudio
    hp2_sdldrv
    hp2_import_core
    hp2_imgui
)
if(NOT EMSCRIPTEN)
    target_link_libraries(hp2_game PRIVATE nfd::nfd)
endif()
if(APPLE)
    target_link_libraries(hp2_game PRIVATE "${HP2_APPKIT_FRAMEWORK}")
else()
    find_package(Threads REQUIRED)
    target_link_libraries(hp2_game PRIVATE Threads::Threads ${CMAKE_DL_LIBS})
endif()

if(HP2_HAS_NATIVE_TEXT_BACKEND)
    # Object libraries do not propagate their objects through other object
    # libraries: consume the shared native-text core directly here.
    target_sources(hp2_game PRIVATE $<TARGET_OBJECTS:hp2_nativetextcore>)
endif()

# ctest cannot execute wasm modules, and the editor/ucc/audit executables exist
# only for native workflows. The browser build skips these; the shared test
# helper, hp2_state, and the data-present gate below stay common, and
# HP2Web.cmake registers the browser smoke tests.
if(NOT EMSCRIPTEN)

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
if(WIN32)
    target_link_libraries(hp2_abi_tests PRIVATE SDL2::SDL2main)
endif()

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
if(NOT EMSCRIPTEN)
    target_link_libraries(hp2_dxt1_tests PRIVATE hp2_ffi)
    target_link_libraries(hp2_eaxa_tests PRIVATE hp2_ffi)
endif()

hp2_add_executable(hp2_audio_tests
    ${HP2_AUDIO_TEST_SOURCE}
)
target_link_libraries(hp2_audio_tests PRIVATE
    hp2_core
    hp2_engine
    hp2_alaudio
    # Tests/AudioTests.cpp's DecodeOgg() is an independent reference
    # decoder using the real libvorbisfile, deliberately kept linked here
    # (not in hp2_core/hp2_alaudio) so the ±1 LSB tolerance this test
    # checks against crates/hp2-codecs::vorbis has a ground truth.
    vorbisfile
)

hp2_add_executable(hp2_launcher_tests
    ${HP2_LAUNCHER_CORE_SOURCES}
    ${HP2_PATH_SOURCE}
    ${HP2_LAUNCHER_TEST_SOURCE}
)
target_compile_definitions(hp2_launcher_tests PRIVATE HP2_LAUNCHER_TESTING=1)
target_link_libraries(hp2_launcher_tests PRIVATE hp2_core hp2_import_core)

# Data-root identity contracts: the two-mode bootstrap rule and the structured
# overlay rejection reason codes (data.manifest_missing, data.hash_mismatch,
# and friends). This executable deliberately does NOT link hp2_core: the test
# supplies its own Core link seams (GMalloc, appSetBaseDir, appFromUtf8InPlace)
# so the bootstrap is exercised in isolation, without appInit or the engine.
hp2_add_executable(hp2_data_identity_tests
    ${HP2_PATH_SOURCE}
    ${HP2_DATA_IDENTITY_TEST_SOURCE}
)

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
if(WIN32)
    target_link_libraries(hp2_input_contract_tests PRIVATE SDL2::SDL2main)
endif()

# SDL main-thread round trip for hp2_rs_platform_get/set_clipboard_text
# (crates/hp2-platform/src/clipboard.rs); see the file's own header
# comment for why this needs a dedicated ctest rather than a Rust-level
# #[test]. Links the same library set as hp2_input_contract_tests above
# (SDLClient.cpp/SDLViewport.cpp's own object code needs hp2_core's
# symbols resolvable, even though this test itself touches no UObject
# machinery) for a known-working combination rather than a guessed
# minimal one.
hp2_add_executable(hp2_platform_clipboard_tests
    ${HP2_PLATFORM_CLIPBOARD_TEST_SOURCE}
)
target_link_libraries(hp2_platform_clipboard_tests PRIVATE
    hp2_core
    hp2_engine
    hp2_render
    hp2_fire
    hp2_editor_runtime
    hp2_sdldrv
)
if(WIN32)
    target_link_libraries(hp2_platform_clipboard_tests PRIVATE SDL2::SDL2main)
endif()

# SDL main-thread checks for hp2_rs_platform_get_viewport_button_flags/
# get_mouse_state/update_mouse_grab_state (crates/hp2-platform/src/
# mouse_state.rs); see the file's own header comment for why this needs
# a dedicated ctest. Same library set as hp2_platform_clipboard_tests
# above, same reason.
hp2_add_executable(hp2_platform_mouse_state_tests
    ${HP2_PLATFORM_MOUSE_STATE_TEST_SOURCE}
)
target_link_libraries(hp2_platform_mouse_state_tests PRIVATE
    hp2_core
    hp2_engine
    hp2_render
    hp2_fire
    hp2_editor_runtime
    hp2_sdldrv
)
if(WIN32)
    target_link_libraries(hp2_platform_mouse_state_tests PRIVATE SDL2::SDL2main)
endif()

# SDL main-thread round trip for hp2_rs_platform_update_window_frame/
# set_title_bar (crates/hp2-platform/src/window_chrome.rs); see the
# file's own header comment for why this needs a dedicated ctest. Same
# library set as hp2_platform_clipboard_tests above, same reason.
hp2_add_executable(hp2_platform_window_chrome_tests
    ${HP2_PLATFORM_WINDOW_CHROME_TEST_SOURCE}
)
target_link_libraries(hp2_platform_window_chrome_tests PRIVATE
    hp2_core
    hp2_engine
    hp2_render
    hp2_fire
    hp2_editor_runtime
    hp2_sdldrv
)
if(WIN32)
    target_link_libraries(hp2_platform_window_chrome_tests PRIVATE SDL2::SDL2main)
endif()

# SDL main-thread checks for hp2_rs_platform_open_joystick/
# close_joystick (crates/hp2-platform/src/joystick.rs); see the
# file's own header comment for why this needs a dedicated ctest. Same
# library set as hp2_platform_clipboard_tests above, same reason.
hp2_add_executable(hp2_platform_joystick_tests
    ${HP2_PLATFORM_JOYSTICK_TEST_SOURCE}
)
target_link_libraries(hp2_platform_joystick_tests PRIVATE
    hp2_core
    hp2_engine
    hp2_render
    hp2_fire
    hp2_editor_runtime
    hp2_sdldrv
)
if(WIN32)
    target_link_libraries(hp2_platform_joystick_tests PRIVATE SDL2::SDL2main)
endif()
if(HP2_HAS_NATIVE_TEXT_BACKEND)
    hp2_add_executable(hp2_native_typography_tests
        ${HP2_NATIVE_TYPOGRAPHY_TEST_SOURCE}
        ${HP2_PATH_SOURCE}
    )
    # hp2_nativetextcore is an OBJECT library: its objects do not survive
    # transitively through another target, so consume them directly here.
    target_sources(hp2_native_typography_tests PRIVATE
        $<TARGET_OBJECTS:hp2_nativetextcore>
    )
    target_link_libraries(hp2_native_typography_tests PRIVATE
        hp2_core
        hp2_engine
        hp2_render
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
    hp2_data_identity_tests
    hp2_config_ini_tests
    hp2_replay_roundtrip_tests
    hp2_save_wire_oracle
    hp2_input_contract_tests
    hp2_platform_clipboard_tests
    hp2_platform_mouse_state_tests
    hp2_platform_window_chrome_tests
    hp2_platform_joystick_tests
)
if(TARGET hp2_import)
    list(APPEND HP2_EXECUTABLE_TARGETS hp2_import)
endif()

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
endif()

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

    add_test(NAME "${test_name}" COMMAND "${target}" ${ARGN})
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
    if(WIN32)
        set(_hp2_test_appdata "${_hp2_test_home}/AppData/Roaming")
        set(_hp2_test_localappdata "${_hp2_test_home}/AppData/Local")
        file(MAKE_DIRECTORY "${_hp2_test_appdata}" "${_hp2_test_localappdata}")
        list(APPEND _hp2_test_env
            "APPDATA=${_hp2_test_appdata}"
            "LOCALAPPDATA=${_hp2_test_localappdata}"
            "USERPROFILE=${_hp2_test_home}"
            "TEMP=${_hp2_test_tmp}"
            "TMP=${_hp2_test_tmp}"
        )
    endif()
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
if(NOT EMSCRIPTEN)
    hp2_add_behavior_test(importer_parity "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/ImporterParityTests.py"
        "$<TARGET_FILE:hp2_import>"
        "${PROJECT_SOURCE_DIR}/Build/prepare_retail_data.py"
    )
    set_tests_properties(importer_parity PROPERTIES LABELS "fast;data-none")
    hp2_add_behavior_test(launcher_smoke "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Tests/LauncherSmoke.py"
        "$<TARGET_FILE:hp2_game>"
    )
    set_tests_properties(launcher_smoke PROPERTIES
        LABELS "smoke;data-none"
        RESOURCE_LOCK hp2_gpu
    )
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

if(NOT EMSCRIPTEN)
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

if(HP2_TEST_DATA_PRESENT)
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
    hp2_add_behavior_test(spell_runtime_contracts hp2_spell_runtime_tests
        "-datadir=${HP2_TEST_DATA_ROOT}"
    )
endif()
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
if(EXISTS "${HP2_UNREAL_ROOT}/Music/sm_bur_PlayfulFail_01.ogg" AND HP2_TEST_DATA_PRESENT)
    hp2_add_behavior_test(audio_lifecycle hp2_audio_tests)
    set_tests_properties(audio_lifecycle PROPERTIES
        DEPENDS "eaxa_decoder;native_registration"
        LABELS "fast;data-prototype"
    )
endif()
if(HP2_TEST_DATA_PRESENT)
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
endif()
hp2_add_behavior_test(native_launcher_contract hp2_launcher_tests)
hp2_add_behavior_test(clipboard_round_trip hp2_platform_clipboard_tests)
hp2_add_behavior_test(mouse_state_round_trip hp2_platform_mouse_state_tests)
hp2_add_behavior_test(window_chrome_round_trip hp2_platform_window_chrome_tests)
hp2_add_behavior_test(joystick_round_trip hp2_platform_joystick_tests)
hp2_add_behavior_test(data_identity_contracts hp2_data_identity_tests)
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


if(HP2_HAS_NATIVE_TEXT_BACKEND AND HP2_TEST_DATA_PRESENT)
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
    hp2_add_behavior_test(renderer_smoke_maps "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Build/smoke_maps.py"
        "--renderer=wgpu"
        "--app=${CMAKE_BINARY_DIR}/hp2_game"
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
            "--app=${CMAKE_BINARY_DIR}/hp2_game"
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

# ABI and registration checks are prerequisites for package/runtime tests.
# Codec tests are otherwise independent; audio lifecycle additionally requires
# the decoder and native registration contracts.
set_tests_properties(compact_index fstring_archive
    PROPERTIES DEPENDS abi_widths
)
if(HP2_TEST_DATA_PRESENT)
    set_tests_properties(native_registration
        PROPERTIES DEPENDS abi_widths
    )
    set_tests_properties(package79_manifest PROPERTIES
        DEPENDS "compact_index;fstring_archive;native_registration"
    )
    set_tests_properties(spell_runtime_contracts PROPERTIES
        DEPENDS "native_registration;package79_manifest;spell_interaction_manifest"
    )
endif()
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
if(HP2_TEST_DATA_PRESENT)
    set_tests_properties(native_registration package79_manifest
        spell_interaction_manifest spell_runtime_contracts
        PROPERTIES LABELS "integration;data-prototype"
    )
endif()
# The typography contracts boot the path bootstrap against real game data, so
# they are data-backed despite running quickly. Labelling them data-none would
# put them in a CI selection that has no data root to give them.
if(HP2_TEST_DATA_PRESENT)
    set_tests_properties(native_typography_contracts
        PROPERTIES LABELS "fast;data-prototype"
    )
endif()
endif()
# ---------------------------------------------------------------------------
# WgpuDrv: experimental wgpu-backed render device.
# hp2-ffi's renderer-wgpu feature is enabled by the corrosion_import_crate block
# above, before this point. One OBJECT library, linked into hp2_game so the engine's
# static class registry finds WgpuDrv.WgpuRenderDevice at boot.
# ---------------------------------------------------------------------------
if(NOT EMSCRIPTEN)
    set(HP2_WGPUDRV_SOURCES
        "${PROJECT_SOURCE_DIR}/HarryPotter2/Unreal/WgpuDrv/Src/WgpuDrv.cpp"
    )
    # hp2_add_engine_object (not bare add_library): IMPLEMENT_CLASS
    # (UnObjBas.h:351-385) references the bare preprocessor token
    # GPackage directly as the package-name argument to UClass's
    # constructor. Only this helper's `GPackage=LocalPackageName${name}`
    # compile definition makes that token resolve to the real symbol
    # IMPLEMENT_PACKAGE(WgpuDrv) defines (LocalPackageNameWgpuDrv).
    # Without it (an earlier version of this block used bare
    # add_library and compiled/linked successfully anyway -- caught on
    # review, not self-caught: a token with no local #define still
    # compiles if it accidentally resolves to some other TU's extern
    # GPackage in scope, it just registers the class under the wrong
    # package silently, not a build failure), WgpuDrv.WgpuRenderDevice
    # would not reliably resolve as an ini class name. Also sets
    # __STATIC_LINK=1 and the HP2_EMPTY_MODULE_API_DEFINITIONS list
    # (WGPUDRV_API= among them) and C++17, matching every other engine
    # object library in this file.
    hp2_add_engine_object(hp2_wgpudrv WgpuDrv ${HP2_WGPUDRV_SOURCES})
    # PRIVATE hp2_ffi, not just a manual HP2_RS_INCLUDE_DIR include dir:
    # without this link, ninja has no dependency edge forcing hp2_ffi's
    # cbindgen-generating custom command to run before WgpuDrv.cpp
    # compiles -- the real cause of this package's first build attempt
    # failing with "undeclared hp2_rs_wgpu_create" even though the
    # header already had the symbol (a prior, separate invocation had
    # already generated it; the retry only "worked" by accident of
    # ordering, not because the graph was correct). hp2_ffi's own
    # INTERFACE include of HP2_RS_INCLUDE_DIR (this file, ~line 227)
    # propagates automatically through this link, same as hp2_core
    # gets it via PUBLIC hp2_ffi -- no separate manual include dir
    # needed or kept here.
    target_link_libraries(hp2_wgpudrv PRIVATE hp2_ffi)
    target_include_directories(hp2_wgpudrv PUBLIC
        ${HP2_RENDER_INCLUDE_DIRS}
        "${PROJECT_SOURCE_DIR}/HarryPotter2/Unreal/WgpuDrv/Inc"
        "${HP2_NATIVETEXT_INCLUDE_DIRS}"
    )

# Static registration: adding the object to hp2_game puts
# WgpuDrv.WgpuRenderDevice into the engine class registry; it is the only
# render device. HP2StaticPackages.cpp's RegisterHP2ClientClasses calls its registrant.
target_link_libraries(hp2_game PRIVATE hp2_wgpudrv)

    if(APPLE)
        # hp2_rs_wgpu_lock/hp2_rs_wgpu_create's window-surface path are
        # only meaningful on macOS (crates/hp2-wgpu/src/lib.rs's
        # `macos` module) -- gate this test target the same way so a
        # hypothetical non-Apple build never
        # tries to compile against a window-surface path that does not
        # exist there.
        set(HP2_WGPU_SURFACE_TEST_SOURCE "${PROJECT_SOURCE_DIR}/Tests/WgpuSurfaceTests.cpp")
        hp2_add_executable(hp2_wgpu_surface_tests
            ${HP2_WGPU_SURFACE_TEST_SOURCE}
        )
        target_link_libraries(hp2_wgpu_surface_tests PRIVATE
            hp2_core
            hp2_engine
            hp2_render
            hp2_fire
            hp2_editor_runtime
            hp2_sdldrv
        )
        # HP2_EXECUTABLE_TARGETS (the hp2_verification_binaries
        # dependency list) is finalized at line ~769-805, long before
        # this WgpuDrv block runs -- adding this target
        # there would need touching that always-on list. Explicit add_dependencies
        # instead: without it, the documented `cmake --build ... --
        # target hp2_verification_binaries && ctest` sequence never
        # builds this executable in a fresh build directory, and
        # wgpu_surface_round_trip fails with "executable not found"
        # (caught on review: an earlier version of this block only
        # "passed" because the executable had already been built by
        # name directly, not through the documented sequence).
        add_dependencies(hp2_verification_binaries hp2_wgpu_surface_tests)
        # Cannot use the "dummy" SDL video driver (see
        # Tests/WgpuSurfaceTests.cpp's own header comment): a real
        # Metal-capable adapter and windowing backend, same
        # RESOURCE_LOCK as renderer_smoke_wgpu below so the two never
        # contend for the GPU concurrently.
        hp2_add_behavior_test(wgpu_surface_round_trip hp2_wgpu_surface_tests)
        set_tests_properties(wgpu_surface_round_trip PROPERTIES
            LABELS "smoke;renderer;wgpu"
            RESOURCE_LOCK hp2_gpu
            # Tests/WgpuSurfaceTests.cpp exits 77 only if its
            # hp2_rs_wgpu_lock check observes a *persistent*
            # FrameOutcome::Occluded after ~2 seconds of polling, with
            # several activation/visibility precautions applied (see
            # that file's own, extensively-commented investigation):
            # wgpu-hal's Metal backend (wgpu-hal-30.0.1/src/metal/
            # surface.rs) refuses to even attempt nextDrawable() unless
            # NSWindow.occlusionState reports the window visible. An
            # earlier comment here claimed this project's own dev
            # environment lacks a window compositor entirely -- wrong,
            # drawn from a `ps aux` grep that does not list every
            # process; corrected once `pmset -g assertions` showed a
            # real WindowServer pid directly. Exactly why that bit does
            # not set without the precautions/polling above was not
            # isolated (no ablation study was run); stated as not
            # isolated, not guessed at further. 77 is the long-standing
            # autotools/CMake skip-exit-code convention.
            SKIP_RETURN_CODE 77
        )
    endif()
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
    )
    set_tests_properties(renderer_smoke_wgpu PROPERTIES
        LABELS "smoke;renderer;wgpu"
        RESOURCE_LOCK hp2_gpu
        TIMEOUT 600
        # No SKIP_RETURN_CODE here, deliberately: run_wgpu_smoke.py's
        # exit 2 ("blocked") now also covers a real -window run that
        # boots and ticks cleanly but never actually presents a frame
        # (data.frame_tally.presented == 0 in its own JSON report,
        # confirmed on a real run on this project's own dev machine --
        # WgpuDrv.h's own doc has the detail). Marking that a skip
        # would hide a real, current gap (no frames ever reach the
        # screen in this environment today) behind a green ctest
        # summary -- AGENTS.md's Forbidden Actions rule out exactly
        # that ("skipping assertions to make a run pass"), and its own
        # "Data-profile honesty" section treats a missing renderer-
        # smoke prerequisite as "a configuration gap, not a skip",
        # which this is closer to than to wgpu_surface_round_trip's
        # SKIP_RETURN_CODE 77 above (a narrowly-scoped, positively-
        # observed environment condition on one specific check, not a
        # blanket cover for the whole smoke test). Left failing on
        # purpose (an earlier version of this change added
        # SKIP_RETURN_CODE 2 here specifically to keep this build
        # dir's ctest count looking stable -- caught on review as the
        # wrong tradeoff before it was ever committed).
    )
endif()
