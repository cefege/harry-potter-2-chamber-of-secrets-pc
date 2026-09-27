include(GNUInstallDirs)

if(APPLE)
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
set(HP2_MACOS_RESOURCES_DIR "${HP2_MACOS_CONTENTS_DIR}/Resources")
set(HP2_IMPORT_SCRIPT
    "${PROJECT_SOURCE_DIR}/Build/prepare_retail_data.py")
set(HP2_IMPORT_LAUNCHER
    "${PROJECT_SOURCE_DIR}/Build/hp2-import-game-data.sh")
set(HP2_MACOS_OPENAL_DYLIB
    "${HP2_MACOS_FRAMEWORKS_DIR}/$<TARGET_SONAME_FILE_NAME:OpenAL>")

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
        "${HP2_MACOS_RESOURCES_DIR}"
    COMMAND "${CMAKE_COMMAND}" -E copy
        "$<TARGET_FILE:hp2_game>"
        "${HP2_MACOS_EXECUTABLE}"
    # Ship the dylib under the soname the executable actually resolves
    # (@rpath/libopenal.1.dylib) instead of its versioned build-tree name plus
    # two symlinks. A quarantine flag propagates onto symlinks when a DMG is
    # downloaded or a cask is installed, which breaks the code seal and makes
    # codesign --verify fail on the *installed* app even though the source
    # bundle is clean. One real file, no symlinks, nothing to propagate onto.
    COMMAND "${CMAKE_COMMAND}" -E copy
        "$<TARGET_FILE:OpenAL>"
        "${HP2_MACOS_OPENAL_DYLIB}"
    COMMAND "${CMAKE_COMMAND}" -E copy
        "${HP2_MACOS_CONFIGURED_PLIST}"
        "${HP2_MACOS_INFO_PLIST}"
    COMMAND "${CMAKE_COMMAND}" -E copy
        "${HP2_IMPORT_SCRIPT}"
        "${HP2_MACOS_RESOURCES_DIR}/prepare_retail_data.py"
    COMMAND "${CMAKE_COMMAND}" -E copy
        "${HP2_IMPORT_LAUNCHER}"
        "${HP2_MACOS_RESOURCES_DIR}/Import Game Data.command"
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

# Distribution disk image.
#
# The release artifact consumed by Homebrew and GitHub Releases. It contains
# exactly the bundle that `hp2_macos_app` produced, so a DMG can never ship a
# different binary than the verified dist/macos-arm64 tree. Retired on every
# invocation: a stale image from an earlier build must never survive.
set(HP2_MACOS_DMG "${HP2_MACOS_DIST_DIR}/HarryPotter2-${PROJECT_VERSION}-macos-arm64.dmg")
set(HP2_DMG_STAGE_DIR "${CMAKE_CURRENT_BINARY_DIR}/Packaging/dmg")

find_program(HP2_HDIUTIL hdiutil)
if(NOT HP2_HDIUTIL)
    message(FATAL_ERROR "hdiutil is required to package the macOS disk image.")
endif()

add_custom_target(hp2_macos_dmg
    COMMAND "${CMAKE_COMMAND}" -E rm -rf "${HP2_DMG_STAGE_DIR}"
    COMMAND "${CMAKE_COMMAND}" -E make_directory
        "${HP2_DMG_STAGE_DIR}"
    COMMAND "${CMAKE_COMMAND}" -E copy_directory
        "${HP2_MACOS_APP_DIR}"
        "${HP2_DMG_STAGE_DIR}/HarryPotter2.app"
    COMMAND "${CMAKE_COMMAND}" -E create_symlink
        "/Applications"
        "${HP2_DMG_STAGE_DIR}/Applications"
    # Plain files at the image root. No symlinked README: it reads as a broken
    # alias for the app rather than an install affordance, and the import
    # command is the thing a first-time user actually needs to reach.
    COMMAND "${CMAKE_COMMAND}" -E copy
        "${PROJECT_SOURCE_DIR}/Docs/PLAYING.md"
        "${HP2_DMG_STAGE_DIR}/How to play.txt"
    COMMAND "${CMAKE_COMMAND}" -E copy
        "${HP2_IMPORT_LAUNCHER}"
        "${HP2_DMG_STAGE_DIR}/Import Game Data.command"
    COMMAND "${CMAKE_COMMAND}" -E copy
        "${HP2_IMPORT_SCRIPT}"
        "${HP2_DMG_STAGE_DIR}/prepare_retail_data.py"
    COMMAND "${HP2_HDIUTIL}" create
        -volname "Harry Potter 2 ${PROJECT_VERSION}"
        -srcfolder "${HP2_DMG_STAGE_DIR}"
        -ov
        -format UDZO
        -quiet
        "${HP2_MACOS_DMG}"
    DEPENDS
        hp2_macos_app
    COMMENT "Creating ${HP2_MACOS_DMG}"
    VERBATIM
)

else()

# Linux build and install: copy the executables and OpenAL next to each
# other with an $ORIGIN-relative RPATH, then assemble a flat dist tree that
# bundles the Quickshell-based launcher UI alongside the game binary.
install(TARGETS ${HP2_EXECUTABLE_TARGETS}
    RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}"
)

install(TARGETS OpenAL
    LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}"
)

set_target_properties(${HP2_EXECUTABLE_TARGETS} PROPERTIES
    INSTALL_RPATH "$ORIGIN/../${CMAKE_INSTALL_LIBDIR}"
)

set_target_properties(hp2_game PROPERTIES
    BUILD_RPATH "$<TARGET_FILE_DIR:OpenAL>"
)

set(HP2_LINUX_DIST_DIR "${PROJECT_SOURCE_DIR}/dist/linux-arm64")

# A separate "hp2_dist" component scopes cmake --install to exactly the
# runtime pieces the dist tree ships (the game binary and its OpenAL
# dependency), never the test suite that also lands in
# HP2_EXECUTABLE_TARGETS. Routing the copy through cmake --install (rather
# than a raw file copy) is what lets CMake's own RPATH_CHANGE rewrite
# hp2_game's BUILD_RPATH to the $ORIGIN-relative INSTALL_RPATH above, so the
# packaged binary stays runnable after the build tree is removed.
install(TARGETS hp2_game
    RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}"
    COMPONENT hp2_dist
    EXCLUDE_FROM_ALL
)
install(TARGETS OpenAL
    LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}"
    COMPONENT hp2_dist
    EXCLUDE_FROM_ALL
)

# Always recreate the distribution: stale binaries and libraries must never
# survive a build invocation. Assets intentionally remain external to the
# dist tree.
add_custom_target(hp2_linux_dist ALL
    COMMAND "${CMAKE_COMMAND}" -E rm -rf "${HP2_LINUX_DIST_DIR}"
    COMMAND "${CMAKE_COMMAND}"
        --install "${CMAKE_BINARY_DIR}"
        --prefix "${HP2_LINUX_DIST_DIR}"
        --component hp2_dist
    COMMAND "${CMAKE_COMMAND}" -E make_directory
        "${HP2_LINUX_DIST_DIR}/share/hp2-launcher"
    COMMAND "${CMAKE_COMMAND}" -E copy_directory
        "${PROJECT_SOURCE_DIR}/Launcher/Quickshell"
        "${HP2_LINUX_DIST_DIR}/share/hp2-launcher"
    DEPENDS
        hp2_game
        OpenAL
    COMMENT "Recreating dist/linux-arm64/"
    VERBATIM
)

endif()
