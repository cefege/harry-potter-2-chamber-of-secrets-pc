include(FetchContent)

# Keep every dependency at the immutable commit recorded in
# ThirdParty/sources.json. Updates are deliberate source changes, never moving
# tags or branches.
if(NOT EMSCRIPTEN)
    FetchContent_Declare(hp2_sdl2
        GIT_REPOSITORY https://github.com/libsdl-org/SDL.git
        GIT_TAG 5d249570393f7a37e037abf22cd6012a4cc56a71
        GIT_SHALLOW FALSE
        GIT_SUBMODULES ""
    )
endif()
if(NOT EMSCRIPTEN)
    FetchContent_Declare(hp2_openal_soft
        GIT_REPOSITORY https://github.com/kcat/openal-soft.git
        GIT_TAG b96a19cfd1705cc81b0bae78c782df1cd39966bd
        GIT_SHALLOW FALSE
        GIT_SUBMODULES ""
    )
endif()
FetchContent_Declare(hp2_ogg
    GIT_REPOSITORY https://github.com/xiph/ogg.git
    GIT_TAG be05b13e98b048f0b5a0f5fa8ce514d56db5f822
    GIT_SHALLOW FALSE
    GIT_SUBMODULES ""
)
FetchContent_Declare(hp2_vorbis
    GIT_REPOSITORY https://github.com/xiph/vorbis.git
    GIT_TAG 0657aee69dec8508a0011f47f3b69d7538e9d262
    GIT_SHALLOW FALSE
    GIT_SUBMODULES ""
)
FetchContent_Declare(hp2_squish
    GIT_REPOSITORY https://github.com/svn2github/libsquish.git
    GIT_TAG c763145a30512c10450954b7a2b5b3a2f9a94e00
    GIT_SHALLOW FALSE
    GIT_SUBMODULES ""
)
# Runtime importer dependencies. All compression libraries are static and
# narrowly configured; archive support is omitted from the Emscripten build.
if(NOT EMSCRIPTEN)
    FetchContent_Declare(hp2_zlib
        GIT_REPOSITORY https://github.com/madler/zlib.git
        GIT_TAG da607da739fa6047df13e66a2af6b8bec7c2a498
    )
    FetchContent_Declare(hp2_bzip2
        GIT_REPOSITORY https://github.com/libarchive/bzip2.git
        GIT_TAG 6a8690fc8d26c815e798c588f796eabe9d684cf0
        GIT_SHALLOW FALSE
    )
    FetchContent_Declare(hp2_xz
        GIT_REPOSITORY https://github.com/tukaani-project/xz.git
        GIT_TAG d3e650e63c110e830fd5391e7f8b45df0b91d3da
    )
    FetchContent_Declare(hp2_libarchive
        GIT_REPOSITORY https://github.com/libarchive/libarchive.git
        GIT_TAG 27cbc7827172698143e440801fc0ba39ccb4f1f5
    )
endif()
FetchContent_Declare(hp2_utf8proc
    GIT_REPOSITORY https://github.com/JuliaStrings/utf8proc.git
    GIT_TAG 79cdd5ab40c79afa559f689ffc13b76812dee1ac
)
FetchContent_Declare(hp2_imgui
    GIT_REPOSITORY https://github.com/ocornut/imgui.git
    GIT_TAG f1cc2ae15e53a861a874c3034aae6798fde194ab
)
if(NOT EMSCRIPTEN)
    FetchContent_Declare(hp2_nativefiledialog
        GIT_REPOSITORY https://github.com/btzy/nativefiledialog-extended.git
        GIT_TAG 65b2f070a0fcfee0d453a64e770a217ab0b6ff4f
    )
endif()
FetchContent_MakeAvailable(hp2_imgui)
if(NOT EMSCRIPTEN)
    set(NFD_BUILD_TESTS OFF CACHE BOOL "" FORCE)
    set(NFD_BUILD_SDL2_TESTS OFF CACHE BOOL "" FORCE)
    set(NFD_BUILD_SDL3_TESTS OFF CACHE BOOL "" FORCE)
    set(NFD_BUILD_GLFW3_TESTS OFF CACHE BOOL "" FORCE)
    set(NFD_INSTALL OFF CACHE BOOL "" FORCE)
    if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
        find_package(PkgConfig QUIET)
        if(PkgConfig_FOUND)
            pkg_check_modules(HP2_GTK3 QUIET gtk+-3.0)
        endif()
        if(NOT HP2_GTK3_FOUND)
            message(FATAL_ERROR "nativefiledialog-extended requires GTK3 development files (gtk+-3.0)")
        endif()
        set(NFD_PORTAL OFF CACHE BOOL "" FORCE)
        set(NFD_X11 ON CACHE BOOL "" FORCE)
        set(NFD_WAYLAND OFF CACHE BOOL "" FORCE)
    endif()
    FetchContent_MakeAvailable(hp2_nativefiledialog)
endif()

if(WIN32)
    FetchContent_Declare(hp2_freetype
        GIT_REPOSITORY https://github.com/freetype/freetype.git
        GIT_TAG 0a0221a1347e2f1e07c395263540026e9a0aa7c7
        GIT_SHALLOW FALSE
    )
    FetchContent_Declare(hp2_harfbuzz
        GIT_REPOSITORY https://github.com/harfbuzz/harfbuzz.git
        GIT_TAG eb033319fc2ebe723597fa497d4bb4331bab8b6b
        GIT_SHALLOW FALSE
    )
    set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
    set(FT_DISABLE_ZLIB ON CACHE BOOL "" FORCE)
    set(FT_DISABLE_BZIP2 ON CACHE BOOL "" FORCE)
    set(FT_DISABLE_PNG ON CACHE BOOL "" FORCE)
    set(FT_DISABLE_HARFBUZZ ON CACHE BOOL "" FORCE)
    set(FT_DISABLE_BROTLI ON CACHE BOOL "" FORCE)
    set(SKIP_INSTALL_ALL ON CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(hp2_freetype)
    set(HB_HAVE_FREETYPE ON CACHE BOOL "" FORCE)
    set(HB_BUILD_UTILS OFF CACHE BOOL "" FORCE)
    set(HB_BUILD_SUBSET OFF CACHE BOOL "" FORCE)
    set(HB_BUILD_RASTER OFF CACHE BOOL "" FORCE)
    set(HB_BUILD_VECTOR OFF CACHE BOOL "" FORCE)
    set(HB_BUILD_GPU OFF CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(hp2_harfbuzz)
endif()

set(UTF8PROC_INSTALL OFF CACHE BOOL "" FORCE)
set(UTF8PROC_ENABLE_TESTING OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(hp2_utf8proc)
if(NOT EMSCRIPTEN)
    set(ZLIB_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
    set(ZLIB_BUILD_TESTING OFF CACHE BOOL "" FORCE)
    set(ZLIB_BUILD_SHARED OFF CACHE BOOL "" FORCE)
    set(ZLIB_BUILD_STATIC ON CACHE BOOL "" FORCE)
    set(ZLIB_INSTALL OFF CACHE BOOL "" FORCE)
    set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(hp2_zlib)
    set(ENABLE_LIB_ONLY ON CACHE BOOL "" FORCE)
    set(ENABLE_TESTING OFF CACHE BOOL "" FORCE)
    set(ENABLE_DOCS OFF CACHE BOOL "" FORCE)
    set(ENABLE_STATIC_LIB ON CACHE BOOL "" FORCE)
    set(ENABLE_SHARED_LIB OFF CACHE BOOL "" FORCE)
    set(BZIP2_ENABLE_EXECUTABLE OFF CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(hp2_bzip2)
    add_library(hp2_bzip2 STATIC
        "${hp2_bzip2_SOURCE_DIR}/blocksort.c"
        "${hp2_bzip2_SOURCE_DIR}/huffman.c"
        "${hp2_bzip2_SOURCE_DIR}/crctable.c"
        "${hp2_bzip2_SOURCE_DIR}/randtable.c"
        "${hp2_bzip2_SOURCE_DIR}/compress.c"
        "${hp2_bzip2_SOURCE_DIR}/decompress.c"
        "${hp2_bzip2_SOURCE_DIR}/bzlib.c"
    )
    target_include_directories(hp2_bzip2 PUBLIC "${hp2_bzip2_SOURCE_DIR}")
    set_target_properties(hp2_bzip2 PROPERTIES POSITION_INDEPENDENT_CODE ON)
    set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
    set(XZ_TOOL_XZ OFF CACHE BOOL "" FORCE)
    set(XZ_TOOL_XZDEC OFF CACHE BOOL "" FORCE)
    set(XZ_TOOL_LZMADEC OFF CACHE BOOL "" FORCE)
    set(XZ_TOOL_LZMAINFO OFF CACHE BOOL "" FORCE)
    set(XZ_DOC OFF CACHE BOOL "" FORCE)
    set(XZ_NLS OFF CACHE BOOL "" FORCE)
    set(XZ_TESTS OFF CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(hp2_xz)
    set(ENABLE_LIBB2 OFF CACHE BOOL "" FORCE)
    set(ENABLE_LZO OFF CACHE BOOL "" FORCE)
    set(ENABLE_MBEDTLS OFF CACHE BOOL "" FORCE)
    set(ENABLE_NETTLE OFF CACHE BOOL "" FORCE)
    set(ENABLE_CNG OFF CACHE BOOL "" FORCE)
    set(ENABLE_LIBGCC OFF CACHE BOOL "" FORCE)
    if(WIN32)
        # libarchive's AUTO regex selection falls through to an optional
        # libgcc-dependent PCRE shim under MinGW. The importer only enables
        # ZIP/TAR/7z readers, none of which requires POSIX regex.
        set(POSIX_REGEX_LIB "LIBC" CACHE STRING "" FORCE)
    endif()
    foreach(_hp2_crypto_impl LIBSYSTEM LIBC LIBC2 LIBC3 LIBMD MBEDTLS NETTLE OPENSSL)
        foreach(_hp2_crypto_alg MD5 RMD160 SHA1 SHA256 SHA384 SHA512)
            set("ARCHIVE_CRYPTO_${_hp2_crypto_alg}_${_hp2_crypto_impl}" OFF CACHE BOOL "" FORCE)
        endforeach()
    endforeach()
    set(ENABLE_LIBXML2 OFF CACHE BOOL "" FORCE)
    set(ENABLE_EXPAT OFF CACHE BOOL "" FORCE)
    set(ENABLE_OPENSSL OFF CACHE BOOL "" FORCE)
    set(ENABLE_LZ4 OFF CACHE BOOL "" FORCE)
    set(ENABLE_ZSTD OFF CACHE BOOL "" FORCE)
    set(ENABLE_BZip2 ON CACHE BOOL "" FORCE)
    set(ENABLE_LZMA ON CACHE BOOL "" FORCE)
    set(ENABLE_ZLIB ON CACHE BOOL "" FORCE)
    set(ENABLE_ACL OFF CACHE BOOL "" FORCE)
    set(ENABLE_XATTR OFF CACHE BOOL "" FORCE)
    set(ENABLE_ICONV OFF CACHE BOOL "" FORCE)
    set(ENABLE_TAR OFF CACHE BOOL "" FORCE)
    set(ENABLE_CPIO OFF CACHE BOOL "" FORCE)
    set(ENABLE_CAT OFF CACHE BOOL "" FORCE)
    set(ENABLE_UNZIP OFF CACHE BOOL "" FORCE)
    set(ENABLE_TEST OFF CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(hp2_libarchive)
    set(_hp2_compression_links "zlibstatic;hp2_bzip2;liblzma")
    set_property(TARGET archive_static PROPERTY LINK_LIBRARIES "${_hp2_compression_links}")
    set_property(TARGET archive_static PROPERTY INTERFACE_LINK_LIBRARIES "${_hp2_compression_links}")
endif()

# InstallShield cabinets: the game's installer CD keeps every game file in
# setup/data1.hdr + dataN.cab. Same pinned source on every platform, built
# from its lib/ sources only (the upstream CMakeLists wants find_package(ZLIB)
# and builds a CLI). SOURCE_SUBDIR points at a directory with no CMakeLists so
# FetchContent only populates.
FetchContent_Declare(hp2_unshield
    GIT_REPOSITORY https://github.com/twogood/unshield.git
    GIT_TAG 51de441ba6893f11026d4671ccef9e8e2a4634fa
    SOURCE_SUBDIR hp2-populate-only
)
FetchContent_MakeAvailable(hp2_unshield)
set(_hp2_unshield_generated "${CMAKE_CURRENT_BINARY_DIR}/hp2_unshield")
file(WRITE "${_hp2_unshield_generated}/lib/unshield_config.h"
    "#define HAVE_INTTYPES_H 1\n#define HAVE_STDBOOL_H 1\n#define HAVE_STDINT_H 1\n"
    "#define HAVE_STDLIB_H 1\n#define HAVE_STRING_H 1\n#define HAVE_SYS_STAT_H 1\n"
    "#define HAVE_SYS_TYPES_H 1\n#define HAVE_UNISTD_H 1\n#define SIZE_FORMAT \"zu\"\n"
    "#define USE_OUR_OWN_MD5 1\n#define PACKAGE \"unshield\"\n#define VERSION \"1.6.2\"\n"
    "#define __USE_MINGW_ANSI_STDIO 1\n")
add_library(hp2_unshield STATIC
    "${hp2_unshield_SOURCE_DIR}/lib/component.c"
    "${hp2_unshield_SOURCE_DIR}/lib/converter.c"
    "${hp2_unshield_SOURCE_DIR}/lib/directory.c"
    "${hp2_unshield_SOURCE_DIR}/lib/file.c"
    "${hp2_unshield_SOURCE_DIR}/lib/file_group.c"
    "${hp2_unshield_SOURCE_DIR}/lib/helper.c"
    "${hp2_unshield_SOURCE_DIR}/lib/libunshield.c"
    "${hp2_unshield_SOURCE_DIR}/lib/log.c"
    "${hp2_unshield_SOURCE_DIR}/lib/md5/md5c.c"
)
target_include_directories(hp2_unshield
    PUBLIC "${hp2_unshield_SOURCE_DIR}/lib"
    PRIVATE "${_hp2_unshield_generated}" "${hp2_unshield_SOURCE_DIR}")
set_target_properties(hp2_unshield PROPERTIES C_STANDARD 99 POSITION_INDEPENDENT_CODE ON)
# RSA's md5/global.h declares MD5Update() without parameters unless
# PROTOTYPES=1, so file.c would pass its 64-bit length unconverted. Native
# ABIs tolerate it; wasm-ld replaces the mismatched call with a trap.
target_compile_definitions(hp2_unshield PRIVATE PROTOTYPES=1)
if(EMSCRIPTEN)
    target_compile_options(hp2_unshield PUBLIC "-sUSE_ZLIB=1")
    target_link_options(hp2_unshield PUBLIC "-sUSE_ZLIB=1")
else()
    target_link_libraries(hp2_unshield PUBLIC zlibstatic)
    target_include_directories(hp2_unshield PRIVATE "${hp2_zlib_SOURCE_DIR}" "${hp2_zlib_BINARY_DIR}")
endif()


# SDL2 is linked statically; its tests, examples, and install surface are not
# part of the HP2 graph.
if(NOT EMSCRIPTEN)
    set(BUILD_SHARED_LIBS OFF CACHE BOOL "Build static third-party libraries" FORCE)
    set(SDL_SHARED OFF CACHE BOOL "" FORCE)
    set(SDL_STATIC ON CACHE BOOL "" FORCE)
    set(SDL_TEST OFF CACHE BOOL "" FORCE)
    set(SDL_TESTS OFF CACHE BOOL "" FORCE)
    set(SDL2_DISABLE_INSTALL ON CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(hp2_sdl2)
else()
    # No browser has a native SDL2; the emsdk port provides one that already
    # binds the HTML5 canvas, keyboard, pointer lock, and WebAudio. Consumers
    # keep linking SDL2::SDL2-static, so no caller changes.
    add_library(SDL2::SDL2-static INTERFACE IMPORTED)
    set_target_properties(SDL2::SDL2-static PROPERTIES
        INTERFACE_COMPILE_OPTIONS "-sUSE_SDL=2"
        INTERFACE_LINK_OPTIONS "-sUSE_SDL=2"
    )
endif()

# Ogg and Vorbis are both static. Vorbis ships a FindOgg module that takes
# precedence over OggConfig.cmake, so seed its cache variables with the pinned
# in-tree target and headers.
set(BUILD_SHARED_LIBS OFF CACHE BOOL "Build static third-party libraries" FORCE)
set(BUILD_FRAMEWORK OFF CACHE BOOL "" FORCE)
set(INSTALL_DOCS OFF CACHE BOOL "" FORCE)
set(INSTALL_PKG_CONFIG_MODULE OFF CACHE BOOL "" FORCE)
set(INSTALL_CMAKE_PACKAGE_MODULE ON CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(hp2_ogg)
set(Ogg_DIR "${hp2_ogg_BINARY_DIR}" CACHE PATH "Pinned in-tree Ogg package" FORCE)
set(OGG_LIBRARY ogg CACHE STRING "Pinned in-tree Ogg target" FORCE)
set(OGG_INCLUDE_DIR "${hp2_ogg_SOURCE_DIR}/include" CACHE PATH
    "Pinned in-tree Ogg headers" FORCE)

set(BUILD_SHARED_LIBS OFF CACHE BOOL "Build static third-party libraries" FORCE)
set(BUILD_FRAMEWORK OFF CACHE BOOL "" FORCE)
set(INSTALL_CMAKE_PACKAGE_MODULE OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(hp2_vorbis)

# The immutable Squish revision defaults to x86 SSE2. Disable that code path on
# arm64 and expose a namespaced alias for the HP2 target declarations.
set(BUILD_SHARED_LIBS OFF CACHE BOOL "Build static third-party libraries" FORCE)
set(BUILD_SQUISH_WITH_SSE2 OFF CACHE BOOL "" FORCE)
set(BUILD_SQUISH_WITH_ALTIVEC OFF CACHE BOOL "" FORCE)
set(BUILD_SQUISH_EXTRA OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(hp2_squish)
if(NOT TARGET Squish::Squish)
    add_library(Squish::Squish ALIAS squish)
endif()

# OpenAL Soft remains a replaceable shared dylib; no utilities, examples, or
# test programs are added to this graph.
if(NOT EMSCRIPTEN)
    set(LIBTYPE SHARED CACHE STRING "OpenAL Soft library type" FORCE)
    set(ALSOFT_UTILS OFF CACHE BOOL "" FORCE)
    set(ALSOFT_EXAMPLES OFF CACHE BOOL "" FORCE)
    set(ALSOFT_TESTS OFF CACHE BOOL "" FORCE)
    set(ALSOFT_INSTALL OFF CACHE BOOL "" FORCE)
    set(ALSOFT_INSTALL_CONFIG OFF CACHE BOOL "" FORCE)
    set(ALSOFT_INSTALL_HRTF_DATA OFF CACHE BOOL "" FORCE)
    set(ALSOFT_INSTALL_AMBDEC_PRESETS OFF CACHE BOOL "" FORCE)
    set(ALSOFT_INSTALL_EXAMPLES OFF CACHE BOOL "" FORCE)
    set(ALSOFT_INSTALL_UTILS OFF CACHE BOOL "" FORCE)
    set(ALSOFT_UPDATE_BUILD_VERSION OFF CACHE BOOL "" FORCE)
    set(ALSOFT_ENABLE_MODULES OFF CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(hp2_openal_soft)
    if(WIN32 AND CMAKE_CXX_COMPILER_ID MATCHES "^(AppleClang|Clang)$")
        # OpenAL Soft 1.25.2's overloaded unique_ptr dereference member pointer
        # does not satisfy libc++'s transform adaptor under llvm-mingw.
        set(_hp2_openal_alc "${hp2_openal_soft_SOURCE_DIR}/alc/alc.cpp")
        file(READ "${_hp2_openal_alc}" _hp2_openal_source)
        set(_hp2_openal_transform "std::views::transform(&ContextBase::EffectSlotCluster::operator*)")
        set(_hp2_openal_transform_compat "std::views::transform([](ContextBase::EffectSlotCluster &cluster) -> decltype(auto) { return *cluster; })")
        string(FIND "${_hp2_openal_source}" "${_hp2_openal_transform}" _hp2_openal_transform_at)
        if(_hp2_openal_transform_at GREATER_EQUAL 0)
            string(REPLACE "${_hp2_openal_transform}" "${_hp2_openal_transform_compat}"
                _hp2_openal_source "${_hp2_openal_source}")
            file(WRITE "${_hp2_openal_alc}" "${_hp2_openal_source}")
        elseif(NOT _hp2_openal_source MATCHES "views::transform\\(\\[\\]\\(ContextBase::EffectSlotCluster")
            message(FATAL_ERROR "OpenAL Soft 1.25.2 transform compatibility patch did not match alc/alc.cpp")
        endif()
    endif()
    if(CMAKE_CXX_COMPILER_ID MATCHES "^(AppleClang|Clang)$")
        # OpenAL Soft 1.25.2 enables -Werror=function-effects on Clang 20+, but
        # Xcode 26 diagnoses its AudioUnit callback lambdas under the newer rules.
        # Keep the warning visible while preventing a pinned dependency from
        # blocking the HP2 build.
        target_compile_options(OpenAL PRIVATE
            "$<$<COMPILE_LANGUAGE:CXX>:-Wno-error=function-effects>")
    endif()
else()
    # Emscripten ships an OpenAL Soft build whose back end is WebAudio. Keep
    # the target name so the audio subsystem links unchanged.
    add_library(OpenAL::OpenAL INTERFACE IMPORTED)
    set_target_properties(OpenAL::OpenAL PROPERTIES
        INTERFACE_LINK_OPTIONS "-lopenal"
    )
endif()

# ---------------------------------------------------------------------------
# Native text backend provider model.
#
# Providers are ordered candidates: hp2_declare_text_provider records one,
# selection below picks the first whose CONDITION evaluates true at configure
# time and exports for callers (never hardcoded paths):
#   HP2_HAS_NATIVE_TEXT_BACKEND  ON when a provider was selected
#   HP2_TEXT_PROVIDER_NAME       selected provider name ("" when none)
#   HP2_TEXT_PROVIDER_SOURCES    provider translation units
#   HP2_TEXT_PROVIDER_LINK_LIBS  libraries the owning driver links PUBLIC
# Adding a backend (DirectWrite, Pango, ...) is one more declare call; no
# caller changes. Capability here only means "compilable", never
# "default-enabled" — runtime gating stays in feature-gates.json.
# ---------------------------------------------------------------------------
set_property(GLOBAL PROPERTY HP2_TEXT_PROVIDERS "")

function(hp2_declare_text_provider name)
    cmake_parse_arguments(PARSE_ARGV 1 _hp2_provider "" "" "SOURCES;LINK_LIBS;CONDITION")
    set_property(GLOBAL APPEND PROPERTY HP2_TEXT_PROVIDERS "${name}")
    set_property(GLOBAL PROPERTY HP2_TEXT_PROVIDER_${name}_SOURCES "${_hp2_provider_SOURCES}")
    set_property(GLOBAL PROPERTY HP2_TEXT_PROVIDER_${name}_LINK_LIBS "${_hp2_provider_LINK_LIBS}")
    set_property(GLOBAL PROPERTY HP2_TEXT_PROVIDER_${name}_CONDITION "${_hp2_provider_CONDITION}")
endfunction()

# Framework probes run unconditionally so each CONDITION is a pure variable
# test; on hosts without the frameworks find_library simply reports NOTFOUND.
find_library(HP2_CORETEXT_FRAMEWORK CoreText)
find_library(HP2_COREGRAPHICS_FRAMEWORK CoreGraphics)

# QUIET pkg-config probes: a host missing the freetype2/harfbuzz/fontconfig
# dev packages must still configure cleanly with the FreeType provider simply
# unmatched, exactly like the CoreText framework probes above.
find_package(PkgConfig QUIET)
if(PkgConfig_FOUND)
    pkg_check_modules(HP2_FREETYPE QUIET IMPORTED_TARGET freetype2)
    pkg_check_modules(HP2_HARFBUZZ QUIET IMPORTED_TARGET harfbuzz)
    pkg_check_modules(HP2_FONTCONFIG QUIET IMPORTED_TARGET fontconfig)
endif()

hp2_declare_text_provider(CoreText
    SOURCES "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/NativeTextShared.cpp"
    LINK_LIBS "${HP2_CORETEXT_FRAMEWORK}" "${HP2_COREGRAPHICS_FRAMEWORK}"
    CONDITION "APPLE AND HP2_CORETEXT_FRAMEWORK AND HP2_COREGRAPHICS_FRAMEWORK"
)
hp2_declare_text_provider(FreeType
    SOURCES "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/NativeTextFreeType.cpp"
    LINK_LIBS PkgConfig::HP2_FREETYPE PkgConfig::HP2_HARFBUZZ PkgConfig::HP2_FONTCONFIG
    CONDITION "NOT APPLE AND NOT WIN32 AND HP2_FREETYPE_FOUND AND HP2_HARFBUZZ_FOUND AND HP2_FONTCONFIG_FOUND"
)
if(WIN32)
    hp2_declare_text_provider(FreeTypeBundled
        SOURCES
            "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/NativeTextFreeType.cpp"
            "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/NativeTextFontsBundled.cpp"
        LINK_LIBS freetype harfbuzz
        CONDITION "WIN32"
    )
endif()

# The browser build uses the same FreeType/HarfBuzz provider as Linux. Only
# font resolution differs -- Fontconfig is a native library with its own font
# cache -- so Emscripten gets the same sources plus a small seam that asks the
# page which font file a family resolves to, and reads it into memory for
# FT_New_Memory_Face. Shaping, fallback, cluster boundaries and the glyph
# cache are unchanged, which is what lets one typography contract cover all
# three builds instead of the web one being a weaker variant.
if(EMSCRIPTEN)
    # Both are Emscripten ports, and neither is in the sysroot by default, so
    # asking emcc to build them is what makes <ft2build.h> and <hb.h> resolve.
    # These are link flags rather than libraries, so they are carried through
    # the provider's global property rather than as target_link_libraries
    # items, and applied to hp2_game at the end (see HP2Targets.cmake).
    set_property(GLOBAL PROPERTY HP2_WEB_FONTLINK_FLAGS
        "-sUSE_FREETYPE=1" "-sUSE_HARFBUZZ=1")
    hp2_declare_text_provider(FreeTypeEmscripten
        SOURCES
            "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/NativeTextFreeType.cpp"
            "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/NativeTextFontSeamEmscripten.cpp"
        LINK_LIBS ""
        CONDITION "TRUE"
    )
endif()

option(HP2_DISABLE_NATIVE_TEXT_BACKEND
    "Force the native text backend off regardless of platform detection \
(the browser build sets this, because no wasm text provider exists yet)" OFF)

set(HP2_HAS_NATIVE_TEXT_BACKEND OFF)
set(HP2_TEXT_PROVIDER_NAME "")
if(NOT HP2_DISABLE_NATIVE_TEXT_BACKEND)
get_property(_hp2_text_provider_candidates GLOBAL PROPERTY HP2_TEXT_PROVIDERS)
foreach(_hp2_text_provider IN LISTS _hp2_text_provider_candidates)
    get_property(_hp2_text_provider_condition GLOBAL
        PROPERTY HP2_TEXT_PROVIDER_${_hp2_text_provider}_CONDITION)
    # Plain if(${condition}) would not re-dereference expanded non-boolean
    # tokens (e.g. framework paths), so evaluate the stored expression with
    # full variable semantics instead.
    set(_hp2_text_provider_matched OFF)
    cmake_language(EVAL CODE "
if(${_hp2_text_provider_condition})
    set(_hp2_text_provider_matched ON)
endif()")
    if(_hp2_text_provider_matched)
        get_property(HP2_TEXT_PROVIDER_SOURCES GLOBAL
            PROPERTY HP2_TEXT_PROVIDER_${_hp2_text_provider}_SOURCES)
        get_property(HP2_TEXT_PROVIDER_LINK_LIBS GLOBAL
            PROPERTY HP2_TEXT_PROVIDER_${_hp2_text_provider}_LINK_LIBS)
        set(HP2_HAS_NATIVE_TEXT_BACKEND ON)
        set(HP2_TEXT_PROVIDER_NAME "${_hp2_text_provider}")
        break()
    endif()
endforeach()
endif()
unset(_hp2_text_provider_candidates)
unset(_hp2_text_provider_matched)
unset(_hp2_text_provider)
unset(_hp2_text_provider_condition)

if(HP2_HAS_NATIVE_TEXT_BACKEND)
    message(STATUS "Native text backend provider: ${HP2_TEXT_PROVIDER_NAME}")
else()
    message(STATUS "Native text backend unavailable: no text provider condition matched")
endif()

if(EMSCRIPTEN)
    # WebGL2 is bound at run time through SDL_GL_GetProcAddress, so no GL
    # library is linked. The version pin keeps Emscripten from emitting the
    # WebGL1 emulation path.
    add_library(OpenGL::GL INTERFACE IMPORTED)
    set_target_properties(OpenGL::GL PROPERTIES INTERFACE_LINK_OPTIONS
        "-sMIN_WEBGL_VERSION=2;-sMAX_WEBGL_VERSION=2;-sGL_ENABLE_GET_PROC_ADDRESS=1"
    )
else()
    find_package(OpenGL REQUIRED)
endif()
if(APPLE)
    find_library(HP2_APPKIT_FRAMEWORK AppKit REQUIRED)
endif()

foreach(_hp2_required_dependency IN ITEMS
    SDL2::SDL2-static
    OpenAL::OpenAL
    Ogg::ogg
    vorbis
    vorbisfile
    Squish::Squish
    OpenGL::GL
)
    if(NOT TARGET "${_hp2_required_dependency}")
        message(FATAL_ERROR "Pinned dependency did not define ${_hp2_required_dependency}")
    endif()
endforeach()
unset(_hp2_required_dependency)
