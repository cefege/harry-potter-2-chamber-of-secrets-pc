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

install(IMPORTED_RUNTIME_ARTIFACTS hp2_import
    RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}"
)

install(TARGETS OpenAL
    LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}"
    RUNTIME DESTINATION "${CMAKE_INSTALL_LIBDIR}"
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

# Diagnose old unsafe cache values without clearing or accepting any reference.
# The packaging target repeats the check immediately before bundle removal.
if(NOT "${HP2_CPP_ORACLE_APP}" STREQUAL "")
    execute_process(
        COMMAND "${Python3_EXECUTABLE}" -B
            "${PROJECT_SOURCE_DIR}/Build/check_oracle_paths.py"
            --oracle "${HP2_CPP_ORACLE_APP}" --destination "${HP2_MACOS_APP_DIR}"
        RESULT_VARIABLE _hp2_oracle_path_result
        ERROR_VARIABLE _hp2_oracle_path_error
    )
    if(NOT "${_hp2_oracle_path_result}" STREQUAL "0")
        message(WARNING
            "Configured HP2_CPP_ORACLE_APP is unsafe or could not be checked; "
            "the value is preserved and packaging remains blocked. "
            "${_hp2_oracle_path_error}")
    endif()
    unset(_hp2_oracle_path_result)
    unset(_hp2_oracle_path_error)
endif()

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
    COMMAND "${Python3_EXECUTABLE}" "${PROJECT_SOURCE_DIR}/Build/check_oracle_paths.py"
        --oracle "${HP2_CPP_ORACLE_APP}" --destination "${HP2_MACOS_APP_DIR}"
    COMMAND "${CMAKE_COMMAND}" -E rm -rf "${HP2_MACOS_APP_DIR}"
    COMMAND "${CMAKE_COMMAND}" -E make_directory
        "${HP2_MACOS_EXECUTABLE_DIR}"
        "${HP2_MACOS_FRAMEWORKS_DIR}"
        "${HP2_MACOS_RESOURCES_DIR}"
    COMMAND "${CMAKE_COMMAND}" -E copy
        "$<TARGET_FILE:hp2_game_rust>"
        "${HP2_MACOS_EXECUTABLE}"
    # Ship the dylib under the soname the executable actually resolves
    # (@rpath/libopenal.1.dylib) instead of its versioned build-tree name plus
    # two symlinks. A quarantine flag propagates onto symlinks when a DMG is
    # downloaded or a cask is installed, which breaks the code seal and makes
    # codesign --verify fail on the *installed* app even though the source
    # bundle is clean. One real file, no symlinks, nothing to propagate onto.
    # The importer ships beside the game: without the launcher it is how a new
    # player imports their own copy of the game (the game shows the command).
    COMMAND "${CMAKE_COMMAND}" -E copy
        "$<TARGET_FILE:hp2_import>"
        "${HP2_MACOS_EXECUTABLE_DIR}/hp2_import"
    COMMAND "${CMAKE_COMMAND}" -E copy
        "$<TARGET_FILE:OpenAL>"
        "${HP2_MACOS_OPENAL_DYLIB}"
    COMMAND "${CMAKE_COMMAND}" -E copy
        "${HP2_MACOS_CONFIGURED_PLIST}"
        "${HP2_MACOS_INFO_PLIST}"
    COMMAND "${CMAKE_COMMAND}" -E copy_directory
        "${PROJECT_SOURCE_DIR}/ThirdParty/fonts"
        "${HP2_MACOS_RESOURCES_DIR}/fonts"
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
        hp2_game_rust
        hp2_import
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
    COMMAND "${CMAKE_COMMAND}" -E copy
        "${PROJECT_SOURCE_DIR}/Docs/PLAYING.md"
        "${HP2_DMG_STAGE_DIR}/How to play.txt"
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

elseif(CMAKE_SYSTEM_NAME STREQUAL "Linux")

# Linux build and install: install the OpenAL dependency and assemble a flat
# dist tree with the shared launcher fonts beside the installed data.
install(IMPORTED_RUNTIME_ARTIFACTS hp2_import
    RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}"
)

install(TARGETS OpenAL
    LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}"
)

corrosion_add_target_local_rustflags(hp2_game_rust
    "-Clink-arg=-Wl,-rpath,\$ORIGIN/../${CMAKE_INSTALL_LIBDIR}")

set(HP2_LINUX_DIST_DIR "${PROJECT_SOURCE_DIR}/dist/linux-${HP2_TARGET_ARCH}")

# A separate "hp2_dist" component scopes cmake --install to exactly the
# runtime pieces the dist tree ships (the game binary and its OpenAL
# dependency). The Rust binary records its $ORIGIN-relative rpath via the
# rustflags above so it stays runnable after the build tree is removed.
install(IMPORTED_RUNTIME_ARTIFACTS hp2_game_rust
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
    COMMAND "${CMAKE_COMMAND}" -E copy_directory
        "${PROJECT_SOURCE_DIR}/ThirdParty/fonts"
        "${HP2_LINUX_DIST_DIR}/share/harry-potter-2/fonts"
    DEPENDS
        hp2_game_rust
        OpenAL
    COMMENT "Recreating dist/linux-${HP2_TARGET_ARCH}/"
    VERBATIM
)
endif()
