include(FetchContent)

# Keep every dependency at the immutable commit recorded in
# ThirdParty/sources.json. Updates are deliberate source changes, never moving
# tags or branches.
    FetchContent_Declare(hp2_sdl2
        GIT_REPOSITORY https://github.com/libsdl-org/SDL.git
        GIT_TAG 5d249570393f7a37e037abf22cd6012a4cc56a71
        GIT_SHALLOW FALSE
        GIT_SUBMODULES ""
    )
    FetchContent_Declare(hp2_openal_soft
        GIT_REPOSITORY https://github.com/kcat/openal-soft.git
        GIT_TAG b2c48f7718ef3fcf67921a8b6534c4914e328970
        GIT_SHALLOW FALSE
        GIT_SUBMODULES ""
    )
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
# Runtime importer dependencies. All compression libraries are static and
# narrowly configured.
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
    FetchContent_Declare(hp2_nativefiledialog
        GIT_REPOSITORY https://github.com/btzy/nativefiledialog-extended.git
        GIT_TAG 65b2f070a0fcfee0d453a64e770a217ab0b6ff4f
    )
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
    # libarchive's FindLibLZMA must compile against the same pinned XZ API
    # that archive_static links below, not an unrelated Homebrew installation.
    set(LIBLZMA_INCLUDE_DIR "${hp2_xz_SOURCE_DIR}/src/liblzma/api"
        CACHE PATH "Pinned XZ API headers for libarchive" FORCE)
    set(ENABLE_LIBB2 OFF CACHE BOOL "" FORCE)
    set(ENABLE_LZO OFF CACHE BOOL "" FORCE)
    set(ENABLE_MBEDTLS OFF CACHE BOOL "" FORCE)
    set(ENABLE_NETTLE OFF CACHE BOOL "" FORCE)
    set(ENABLE_CNG OFF CACHE BOOL "" FORCE)
    set(ENABLE_LIBGCC OFF CACHE BOOL "" FORCE)
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

# Corrosion imports the Rust workspace (Cargo.toml at the repository root) so
# the hp2-ffi staticlib builds as part of this graph. Its own tests only build
# when Corrosion is the top-level project. rust-toolchain.toml is the one
# toolchain pin: Rust_TOOLCHAIN makes Corrosion resolve it through rustup
# instead of whichever rustc comes first on PATH.
    file(STRINGS "${PROJECT_SOURCE_DIR}/rust-toolchain.toml" _hp2_rust_channel
        REGEX "^channel = \"[^\"]+\"$")
    string(REGEX REPLACE "^channel = \"([^\"]+)\"$" "\\1" _hp2_rust_channel "${_hp2_rust_channel}")
    if(NOT _hp2_rust_channel)
        message(FATAL_ERROR "rust-toolchain.toml has no channel pin")
    endif()
    set(Rust_TOOLCHAIN "${_hp2_rust_channel}" CACHE STRING "rustup toolchain (from rust-toolchain.toml)" FORCE)
    FetchContent_Declare(hp2_corrosion
        GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
        GIT_TAG 1499b14e4906a2890f5cee1547c8848db261753d
    )
    FetchContent_MakeAvailable(hp2_corrosion)


# SDL2 is linked statically; its tests, examples, and install surface are not
# part of the HP2 graph.
    set(BUILD_SHARED_LIBS OFF CACHE BOOL "Build static third-party libraries" FORCE)
    set(SDL_SHARED OFF CACHE BOOL "" FORCE)
    set(SDL_STATIC ON CACHE BOOL "" FORCE)
    set(SDL_TEST OFF CACHE BOOL "" FORCE)
    set(SDL_TESTS OFF CACHE BOOL "" FORCE)
    set(SDL2_DISABLE_INSTALL ON CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(hp2_sdl2)

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

# OpenAL Soft remains a replaceable shared dylib; no utilities, examples, or
# test programs are added to this graph.
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
    if(CMAKE_CXX_COMPILER_ID MATCHES "^(AppleClang|Clang)$")
        # OpenAL Soft 1.25.2 enables -Werror=function-effects on Clang 20+, but
        # Xcode 26 diagnoses its AudioUnit callback lambdas under the newer rules.
        # Keep the warning visible while preventing a pinned dependency from
        # blocking the HP2 build.
        target_compile_options(OpenAL PRIVATE
            "$<$<COMPILE_LANGUAGE:CXX>:-Wno-error=function-effects>")
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
    SOURCES "${HP2_UNREAL_ROOT}/NativeText/Src/NativeTextShared.cpp"
    LINK_LIBS "${HP2_CORETEXT_FRAMEWORK}" "${HP2_COREGRAPHICS_FRAMEWORK}"
    CONDITION "APPLE AND HP2_CORETEXT_FRAMEWORK AND HP2_COREGRAPHICS_FRAMEWORK"
)
hp2_declare_text_provider(FreeType
    SOURCES "${HP2_UNREAL_ROOT}/NativeText/Src/NativeTextFreeType.cpp"
    LINK_LIBS PkgConfig::HP2_FREETYPE PkgConfig::HP2_HARFBUZZ PkgConfig::HP2_FONTCONFIG
    CONDITION "NOT APPLE AND HP2_FREETYPE_FOUND AND HP2_HARFBUZZ_FOUND AND HP2_FONTCONFIG_FOUND"
)

option(HP2_DISABLE_NATIVE_TEXT_BACKEND
    "Force the native text backend off regardless of platform detection" OFF)

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

find_package(OpenGL REQUIRED)
if(APPLE)
    find_library(HP2_APPKIT_FRAMEWORK AppKit REQUIRED)
endif()

foreach(_hp2_required_dependency IN ITEMS
    SDL2::SDL2-static
    OpenAL::OpenAL
    Ogg::ogg
    vorbis
    vorbisfile
    OpenGL::GL
)
    if(NOT TARGET "${_hp2_required_dependency}")
        message(FATAL_ERROR "Pinned dependency did not define ${_hp2_required_dependency}")
    endif()
endforeach()
unset(_hp2_required_dependency)
