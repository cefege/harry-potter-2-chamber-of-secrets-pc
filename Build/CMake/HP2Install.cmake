include(GNUInstallDirs)

# OpenAL Soft is the sole replaceable runtime dependency in this phase. Give
# the build dylib its eventual bundle-relative identity so every executable
# records @rpath/libopenal.1.dylib rather than an absolute build-tree path.
set_target_properties(OpenAL PROPERTIES
    BUILD_WITH_INSTALL_NAME_DIR TRUE
    INSTALL_NAME_DIR "@rpath"
    MACOSX_RPATH TRUE
)

install(TARGETS ${HP2_EXECUTABLE_TARGETS}
    RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}"
)

install(TARGETS OpenAL
    LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}"
    RUNTIME DESTINATION "${CMAKE_INSTALL_LIBDIR}"
)

set_target_properties(${HP2_EXECUTABLE_TARGETS} PROPERTIES
    INSTALL_RPATH "@loader_path/../${CMAKE_INSTALL_LIBDIR}"
)

# Keep direct build-tree launches working while ensuring the packaged copy has
# only the bundle-relative search path. The package target replaces this exact
# build RPATH after copying the executable.
set_target_properties(hp2_game PROPERTIES
    BUILD_RPATH "$<TARGET_FILE_DIR:OpenAL>"
    INSTALL_RPATH "@executable_path/../Frameworks"
)

find_program(HP2_INSTALL_NAME_TOOL
    NAMES install_name_tool
    REQUIRED
)
find_program(HP2_CODESIGN
    NAMES codesign
    REQUIRED
)

set(HP2_MACOS_DIST_DIR "${PROJECT_SOURCE_DIR}/dist/macos-arm64")
set(HP2_MACOS_APP_DIR "${HP2_MACOS_DIST_DIR}/HarryPotter2.app")
set(HP2_MACOS_CONTENTS_DIR "${HP2_MACOS_APP_DIR}/Contents")
set(HP2_MACOS_EXECUTABLE_DIR "${HP2_MACOS_CONTENTS_DIR}/MacOS")
set(HP2_MACOS_FRAMEWORKS_DIR "${HP2_MACOS_CONTENTS_DIR}/Frameworks")
set(HP2_MACOS_EXECUTABLE "${HP2_MACOS_EXECUTABLE_DIR}/HarryPotter2")
set(HP2_MACOS_INFO_PLIST "${HP2_MACOS_CONTENTS_DIR}/Info.plist")
set(HP2_MACOS_OPENAL_DYLIB
    "${HP2_MACOS_FRAMEWORKS_DIR}/$<TARGET_FILE_NAME:OpenAL>")
set(HP2_MACOS_OPENAL_SONAME_LINK
    "${HP2_MACOS_FRAMEWORKS_DIR}/$<TARGET_SONAME_FILE_NAME:OpenAL>")
set(HP2_MACOS_OPENAL_LINKER_LINK
    "${HP2_MACOS_FRAMEWORKS_DIR}/$<TARGET_LINKER_FILE_NAME:OpenAL>")

file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/Packaging")
set(HP2_MACOS_CONFIGURED_PLIST
    "${CMAKE_CURRENT_BINARY_DIR}/Packaging/HarryPotter2-Info.plist")
configure_file(
    "${CMAKE_CURRENT_LIST_DIR}/HarryPotter2-Info.plist.in"
    "${HP2_MACOS_CONFIGURED_PLIST}"
    @ONLY
)

# The documented quick-start flow is `cmake --build --preset macos-arm64`, so
# the release bundle belongs to the default target. Sanitizer configurations
# share dist/macos-arm64 and must never silently replace the shipped release
# bundle with an instrumented binary; they package on explicit request only.
if(HP2_ENABLE_ASAN_UBSAN OR HP2_ENABLE_TSAN)
    set(_hp2_bundle_in_all "")
else()
    set(_hp2_bundle_in_all ALL)
endif()

# Always recreate the bundle: stale binaries, signatures, and future runtime
# libraries must never survive a packaging invocation. Assets intentionally
# remain external to the application bundle.
add_custom_target(hp2_macos_app ${_hp2_bundle_in_all}
    COMMAND "${CMAKE_COMMAND}" -E rm -rf "${HP2_MACOS_APP_DIR}"
    COMMAND "${CMAKE_COMMAND}" -E make_directory
        "${HP2_MACOS_EXECUTABLE_DIR}"
        "${HP2_MACOS_FRAMEWORKS_DIR}"
    COMMAND "${CMAKE_COMMAND}" -E copy
        "$<TARGET_FILE:hp2_game>"
        "${HP2_MACOS_EXECUTABLE}"
    COMMAND "${CMAKE_COMMAND}" -E copy
        "$<TARGET_FILE:OpenAL>"
        "${HP2_MACOS_OPENAL_DYLIB}"
    COMMAND "${CMAKE_COMMAND}" -E create_symlink
        "$<TARGET_FILE_NAME:OpenAL>"
        "${HP2_MACOS_OPENAL_SONAME_LINK}"
    COMMAND "${CMAKE_COMMAND}" -E create_symlink
        "$<TARGET_SONAME_FILE_NAME:OpenAL>"
        "${HP2_MACOS_OPENAL_LINKER_LINK}"
    COMMAND "${CMAKE_COMMAND}" -E copy
        "${HP2_MACOS_CONFIGURED_PLIST}"
        "${HP2_MACOS_INFO_PLIST}"
    COMMAND "${HP2_INSTALL_NAME_TOOL}"
        -id "@rpath/$<TARGET_SONAME_FILE_NAME:OpenAL>"
        "${HP2_MACOS_OPENAL_DYLIB}"
    COMMAND "${HP2_INSTALL_NAME_TOOL}"
        -rpath "$<TARGET_FILE_DIR:OpenAL>"
        "@executable_path/../Frameworks"
        "${HP2_MACOS_EXECUTABLE}"
    COMMAND "${HP2_CODESIGN}"
        --force
        --deep
        --sign -
        --timestamp=none
        "${HP2_MACOS_APP_DIR}"
    DEPENDS
        hp2_game
        OpenAL
    COMMENT "Recreating signed dist/macos-arm64/HarryPotter2.app"
    VERBATIM
)
unset(_hp2_bundle_in_all)
