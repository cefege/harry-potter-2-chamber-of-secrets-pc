if(NOT DEFINED ENV{LLVM_MINGW_ROOT} OR "$ENV{LLVM_MINGW_ROOT}" STREQUAL "")
  message(FATAL_ERROR "LLVM_MINGW_ROOT must point to the llvm-mingw installation root.")
endif()

set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR x86_64)

set(_HP2_MINGW_EXECUTABLE_SUFFIX "")
if(CMAKE_HOST_WIN32)
  set(_HP2_MINGW_EXECUTABLE_SUFFIX ".exe")
endif()
# CMake form (forward slashes): CMake writes these paths into generated
# .cmake files, where a Windows path like D:\a\_temp would be read as escapes.
string(REPLACE "\\" "/" HP2_LLVM_MINGW_ROOT "$ENV{LLVM_MINGW_ROOT}")
set(CMAKE_C_COMPILER "${HP2_LLVM_MINGW_ROOT}/bin/x86_64-w64-mingw32-clang${_HP2_MINGW_EXECUTABLE_SUFFIX}")
set(CMAKE_CXX_COMPILER "${HP2_LLVM_MINGW_ROOT}/bin/x86_64-w64-mingw32-clang++${_HP2_MINGW_EXECUTABLE_SUFFIX}")
set(CMAKE_RC_COMPILER "${HP2_LLVM_MINGW_ROOT}/bin/x86_64-w64-mingw32-windres${_HP2_MINGW_EXECUTABLE_SUFFIX}")

set(CMAKE_FIND_ROOT_PATH "${HP2_LLVM_MINGW_ROOT}/x86_64-w64-mingw32")
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
