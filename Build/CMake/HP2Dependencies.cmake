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
FetchContent_Declare(hp2_squish
    GIT_REPOSITORY https://github.com/svn2github/libsquish.git
    GIT_TAG c763145a30512c10450954b7a2b5b3a2f9a94e00
    GIT_SHALLOW FALSE
    GIT_SUBMODULES ""
)

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

hp2_declare_text_provider(CoreText
    SOURCES "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/NativeText.cpp"
    LINK_LIBS "${HP2_CORETEXT_FRAMEWORK}" "${HP2_COREGRAPHICS_FRAMEWORK}"
    CONDITION "APPLE AND HP2_CORETEXT_FRAMEWORK AND HP2_COREGRAPHICS_FRAMEWORK"
)

set(HP2_HAS_NATIVE_TEXT_BACKEND OFF)
set(HP2_TEXT_PROVIDER_NAME "")
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
        list(APPEND HP2_XOPENGLDRV_SOURCES ${HP2_TEXT_PROVIDER_SOURCES})
        break()
    endif()
endforeach()
unset(_hp2_text_provider_candidates)
unset(_hp2_text_provider_matched)
unset(_hp2_text_provider)
unset(_hp2_text_provider_condition)

if(NOT HP2_HAS_NATIVE_TEXT_BACKEND)
    message(STATUS "Native text backend unavailable: no text provider condition matched")
endif()

find_package(OpenGL REQUIRED)
find_library(HP2_APPKIT_FRAMEWORK AppKit REQUIRED)

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
