cmake_minimum_required(VERSION 3.24)

project(HarryPotter2Native
    VERSION 0.1.0
    LANGUAGES C CXX OBJC OBJCXX
)

if(NOT APPLE OR NOT CMAKE_SYSTEM_NAME STREQUAL "Darwin")
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

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# libSquish still declares compatibility with CMake 2.8. CMake 4 requires an
# explicit compatibility floor while this immutable dependency is in use.
set(CMAKE_POLICY_VERSION_MINIMUM 3.5 CACHE STRING
    "Minimum policy compatibility accepted for pinned third-party projects" FORCE)

option(HP2_REQUIRE_IMPLEMENTED_SOURCES
    "Fail configuration unless every planned step-owned source has been implemented"
    ON
)
option(HP2_ENABLE_ASAN_UBSAN "Enable AddressSanitizer and UndefinedBehaviorSanitizer" OFF)
option(HP2_ENABLE_TSAN "Enable ThreadSanitizer" OFF)

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
