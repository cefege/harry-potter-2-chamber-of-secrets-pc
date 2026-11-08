cmake_minimum_required(VERSION 3.24)

include("${CMAKE_CURRENT_LIST_DIR}/Build/CMake/HP2NativeFree.cmake")

project(HarryPotter2Native
    VERSION 0.1.0
    LANGUAGES C CXX
)

if(APPLE)
    set(HP2_TARGET_ARCH arm64 CACHE INTERNAL "Target architecture")
    enable_language(OBJC)
    enable_language(OBJCXX)

    if(NOT CMAKE_SYSTEM_NAME STREQUAL "Darwin")
        message(FATAL_ERROR "HarryPotter2Native supports Darwin only.")
    endif()

    if(NOT CMAKE_HOST_SYSTEM_NAME STREQUAL "Darwin" OR
       NOT CMAKE_HOST_SYSTEM_PROCESSOR MATCHES "^(arm64|aarch64)$")
        message(FATAL_ERROR "HarryPotter2Native must be configured natively on an arm64 Darwin host.")
    endif()

    if(NOT CMAKE_OSX_ARCHITECTURES STREQUAL "arm64")
        message(FATAL_ERROR
            "HarryPotter2Native is arm64-only; configure with CMAKE_OSX_ARCHITECTURES=arm64.")
    endif()

    if(NOT CMAKE_OSX_DEPLOYMENT_TARGET STREQUAL "15.0")
        message(FATAL_ERROR
            "HarryPotter2Native targets macOS 15.0; configure with CMAKE_OSX_DEPLOYMENT_TARGET=15.0.")
    endif()
elseif(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    if(NOT CMAKE_SYSTEM_PROCESSOR MATCHES "^(aarch64|arm64|x86_64|AMD64)$")
        message(FATAL_ERROR "HarryPotter2Native supports Linux on aarch64 and x86_64 only.")
    endif()
    if(CMAKE_SYSTEM_PROCESSOR MATCHES "^(aarch64|arm64)$")
        set(HP2_TARGET_ARCH arm64 CACHE INTERNAL "Target architecture")
    else()
        set(HP2_TARGET_ARCH x64 CACHE INTERNAL "Target architecture")
    endif()
else()
    message(FATAL_ERROR "HarryPotter2Native supports Darwin and Linux only.")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

option(HP2_ENABLE_ASAN_UBSAN "Enable AddressSanitizer and UndefinedBehaviorSanitizer" OFF)
option(HP2_ENABLE_TSAN "Enable ThreadSanitizer" OFF)
option(HP2_ENABLE_FULL_MAP_SMOKE
    "Register the full playable-map renderer sweep (long; opt-in preset)"
    OFF
)

if(HP2_ENABLE_ASAN_UBSAN AND HP2_ENABLE_TSAN)
    message(FATAL_ERROR "ASan+UBSan and TSan configurations are mutually exclusive.")
endif()

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/Build/CMake")

include(HP2Sources)

# Keep FetchContent projects from registering their own test suites. CTest is
# enabled only after the pinned dependencies have been configured so the root
# project owns the complete, stable set of behavioral tests.
set(BUILD_TESTING OFF CACHE BOOL "Disable third-party tests" FORCE)
include(HP2Dependencies)
set(BUILD_TESTING ON CACHE BOOL "Build HP2 behavioral tests" FORCE)
include(CTest)

include(HP2Targets)
include(HP2Install)

# Record the mixed products as well as the Rust importer. Recording a product
# does not certify it: native engine owners must fail the dependency guard.
hp2_native_free_manifest(
    OUTPUT "${CMAKE_BINARY_DIR}/hp2-native-free-manifest.json"
    PRODUCTS hp2_game_rust hp2_ucc_rust hp2_import
)

if(DEFINED ENV{RUSTUP_HOME} AND NOT "$ENV{RUSTUP_HOME}" STREQUAL "")
    set(_hp2_native_free_rustup_home "$ENV{RUSTUP_HOME}")
else()
    set(_hp2_native_free_rustup_home "$ENV{HOME}/.rustup")
endif()
hp2_add_behavior_test(native_free_import "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "RUSTUP_HOME=${_hp2_native_free_rustup_home}"
    "RUSTC_WRAPPER="
    "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Build/check_native_free.py"
    --build-dir "${CMAKE_BINARY_DIR}"
    --product hp2_import
    --report "${CMAKE_BINARY_DIR}/Testing/HP2/native_free_import/Artifacts/native-free.json"
)
hp2_add_behavior_test(native_free_ucc "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "RUSTUP_HOME=${_hp2_native_free_rustup_home}"
    "RUSTC_WRAPPER="
    "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Build/check_native_free.py"
    --build-dir "${CMAKE_BINARY_DIR}"
    --product hp2_ucc_rust
    --report "${CMAKE_BINARY_DIR}/Testing/HP2/native_free_ucc/Artifacts/native-free.json"
)
hp2_add_behavior_test(native_free_game "${CMAKE_COMMAND}" -E env
    "CARGO_HOME=${HP2_CARGO_HOME}"
    "RUSTUP_HOME=${_hp2_native_free_rustup_home}"
    "RUSTC_WRAPPER="
    "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Build/check_native_free.py"
    --build-dir "${CMAKE_BINARY_DIR}"
    --product hp2_game_rust
    --report "${CMAKE_BINARY_DIR}/Testing/HP2/native_free_game/Artifacts/native-free.json"
)
hp2_add_behavior_test(native_free_producer_contracts
    "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Tests/NativeFreeProducerTests.py"
)
