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

# Compile Icon Composer's source package with the active Xcode toolchain.
find_program(HP2_XCRUN NAMES xcrun REQUIRED)
set(HP2_MACOS_ICON_SOURCE "${PROJECT_SOURCE_DIR}/Build/Resources/HP2.icon")

set(HP2_MACOS_DIST_DIR "${PROJECT_SOURCE_DIR}/dist/macos-arm64")
set(HP2_MACOS_APP_DIR "${HP2_MACOS_DIST_DIR}/HarryPotter2.app")
set(HP2_MACOS_CONTENTS_DIR "${HP2_MACOS_APP_DIR}/Contents")
set(HP2_MACOS_EXECUTABLE_DIR "${HP2_MACOS_CONTENTS_DIR}/MacOS")
set(HP2_MACOS_FRAMEWORKS_DIR "${HP2_MACOS_CONTENTS_DIR}/Frameworks")
set(HP2_MACOS_EXECUTABLE "${HP2_MACOS_EXECUTABLE_DIR}/HarryPotter2")
set(HP2_MACOS_INFO_PLIST "${HP2_MACOS_CONTENTS_DIR}/Info.plist")
set(HP2_MACOS_RESOURCES_DIR "${HP2_MACOS_CONTENTS_DIR}/Resources")
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
    COMMAND "${CMAKE_COMMAND}" -E copy_directory
        "${PROJECT_SOURCE_DIR}/ThirdParty/fonts"
        "${HP2_MACOS_RESOURCES_DIR}/fonts"
    COMMAND "${HP2_XCRUN}" actool
        --compile "${HP2_MACOS_RESOURCES_DIR}"
        --platform macosx
        --minimum-deployment-target 15.0
        --app-icon HP2
        --output-partial-info-plist "${CMAKE_CURRENT_BINARY_DIR}/Packaging/HP2-Icon.plist"
        "${HP2_MACOS_ICON_SOURCE}"
    COMMAND "${HP2_INSTALL_NAME_TOOL}"
        -id "@rpath/$<TARGET_SONAME_FILE_NAME:OpenAL>"
        "${HP2_MACOS_OPENAL_DYLIB}"
    COMMAND "${HP2_INSTALL_NAME_TOOL}"
        -rpath "$<TARGET_FILE_DIR:OpenAL>"
        "@executable_path/../Frameworks"
        "${HP2_MACOS_EXECUTABLE}"
    COMMAND "${HP2_CODESIGN}"
        --force --sign - --timestamp=none "${HP2_MACOS_OPENAL_DYLIB}"
    COMMAND "${HP2_CODESIGN}"
        --force --sign - --timestamp=none "${HP2_MACOS_APP_DIR}"
    COMMAND "${HP2_CODESIGN}"
        --verify --deep --strict "${HP2_MACOS_APP_DIR}"
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

# Linux build and install: copy the executables and OpenAL next to each
# other with an $ORIGIN-relative RPATH, then assemble a flat dist tree with
# the shared launcher fonts beside the installed data.
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

set(HP2_LINUX_DIST_DIR "${PROJECT_SOURCE_DIR}/dist/linux-${HP2_TARGET_ARCH}")

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
    COMMAND "${CMAKE_COMMAND}" -E copy_directory
        "${PROJECT_SOURCE_DIR}/ThirdParty/fonts"
        "${HP2_LINUX_DIST_DIR}/share/harry-potter-2/fonts"
    DEPENDS
        hp2_game
        OpenAL
    COMMENT "Recreating dist/linux-${HP2_TARGET_ARCH}/"
    VERBATIM
)

elseif(WIN32)
set(HP2_WINDOWS_DIST_DIR "${PROJECT_SOURCE_DIR}/dist/windows-x64")
set(HP2_WINDOWS_RUNTIME_DIR "$ENV{LLVM_MINGW_ROOT}/x86_64-w64-mingw32/bin")
find_program(HP2_LLVM_OBJDUMP NAMES llvm-objdump
    HINTS "$ENV{LLVM_MINGW_ROOT}/bin" "${HP2_WINDOWS_RUNTIME_DIR}"
    REQUIRED
)

install(TARGETS hp2_game
    RUNTIME DESTINATION "."
    COMPONENT hp2_windows_dist
    EXCLUDE_FROM_ALL
)
install(TARGETS OpenAL
    RUNTIME DESTINATION "."
    COMPONENT hp2_windows_dist
    EXCLUDE_FROM_ALL
)

add_custom_target(hp2_windows_dist
    COMMAND "${CMAKE_COMMAND}" -E rm -rf "${HP2_WINDOWS_DIST_DIR}"
    COMMAND "${CMAKE_COMMAND}" --install "${CMAKE_BINARY_DIR}"
        --prefix "${HP2_WINDOWS_DIST_DIR}" --component hp2_windows_dist
    COMMAND "${CMAKE_COMMAND}" -E copy_directory
        "${PROJECT_SOURCE_DIR}/ThirdParty/fonts" "${HP2_WINDOWS_DIST_DIR}/fonts"
    COMMAND "${CMAKE_COMMAND}" -E make_directory "${HP2_WINDOWS_DIST_DIR}/licenses"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_sdl2_SOURCE_DIR}/LICENSE.txt" "${HP2_WINDOWS_DIST_DIR}/licenses/SDL2-LICENSE.txt"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_openal_soft_SOURCE_DIR}/COPYING" "${HP2_WINDOWS_DIST_DIR}/licenses/OpenAL-COPYING"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_openal_soft_SOURCE_DIR}/BSD-3Clause" "${HP2_WINDOWS_DIST_DIR}/licenses/OpenAL-BSD-3Clause"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_openal_soft_SOURCE_DIR}/LICENSE-pffft" "${HP2_WINDOWS_DIST_DIR}/licenses/OpenAL-LICENSE-pffft"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_ogg_SOURCE_DIR}/COPYING" "${HP2_WINDOWS_DIST_DIR}/licenses/libogg-COPYING"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_vorbis_SOURCE_DIR}/COPYING" "${HP2_WINDOWS_DIST_DIR}/licenses/libvorbis-COPYING"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_squish_SOURCE_DIR}/LICENSE" "${HP2_WINDOWS_DIST_DIR}/licenses/libSquish-LICENSE"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_imgui_SOURCE_DIR}/LICENSE.txt" "${HP2_WINDOWS_DIST_DIR}/licenses/ImGui-LICENSE.txt"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_nativefiledialog_SOURCE_DIR}/LICENSE" "${HP2_WINDOWS_DIST_DIR}/licenses/nativefiledialog-LICENSE"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_libarchive_SOURCE_DIR}/COPYING" "${HP2_WINDOWS_DIST_DIR}/licenses/libarchive-COPYING"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_zlib_SOURCE_DIR}/LICENSE" "${HP2_WINDOWS_DIST_DIR}/licenses/zlib-LICENSE"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_bzip2_SOURCE_DIR}/LICENSE" "${HP2_WINDOWS_DIST_DIR}/licenses/bzip2-LICENSE"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_xz_SOURCE_DIR}/COPYING" "${HP2_WINDOWS_DIST_DIR}/licenses/xz-COPYING"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_xz_SOURCE_DIR}/COPYING.GPLv2" "${HP2_WINDOWS_DIST_DIR}/licenses/xz-COPYING.GPLv2"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_xz_SOURCE_DIR}/COPYING.0BSD" "${HP2_WINDOWS_DIST_DIR}/licenses/xz-COPYING.0BSD"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_xz_SOURCE_DIR}/COPYING.LGPLv2.1" "${HP2_WINDOWS_DIST_DIR}/licenses/xz-COPYING.LGPLv2.1"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_utf8proc_SOURCE_DIR}/LICENSE.md" "${HP2_WINDOWS_DIST_DIR}/licenses/utf8proc-LICENSE.md"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_freetype_SOURCE_DIR}/LICENSE.TXT" "${HP2_WINDOWS_DIST_DIR}/licenses/FreeType-LICENSE.TXT"
    COMMAND "${CMAKE_COMMAND}" -E copy "${hp2_harfbuzz_SOURCE_DIR}/COPYING" "${HP2_WINDOWS_DIST_DIR}/licenses/HarfBuzz-COPYING"
    COMMAND "${CMAKE_COMMAND}" -E copy "${PROJECT_SOURCE_DIR}/ThirdParty/XOpenGLDrv/LICENSE.md" "${HP2_WINDOWS_DIST_DIR}/licenses/XOpenGLDrv-LICENSE.md"
    COMMAND "${CMAKE_COMMAND}" -E copy "${PROJECT_SOURCE_DIR}/ThirdParty/XOpenGLDrv/glm/copying.txt" "${HP2_WINDOWS_DIST_DIR}/licenses/GLM-copying.txt"
    COMMAND "${CMAKE_COMMAND}" -E copy "${PROJECT_SOURCE_DIR}/ThirdParty/fonts/LICENSE-Liberation.txt" "${HP2_WINDOWS_DIST_DIR}/licenses/Liberation-LICENSE.txt"
    COMMAND "${CMAKE_COMMAND}" -E copy "${PROJECT_SOURCE_DIR}/ThirdParty/fonts/LICENSE-NotoSansCJK.txt" "${HP2_WINDOWS_DIST_DIR}/licenses/NotoSansCJK-LICENSE.txt"
    COMMAND "${CMAKE_COMMAND}"
        "-DHP2_DIST_DIR=${HP2_WINDOWS_DIST_DIR}"
        "-DHP2_RUNTIME_DLL_DIR=${HP2_WINDOWS_RUNTIME_DIR}"
        "-DHP2_OBJDUMP=${HP2_LLVM_OBJDUMP}"
        -P "${CMAKE_CURRENT_LIST_DIR}/CheckWindowsRuntime.cmake"
    DEPENDS hp2_game OpenAL
    COMMENT "Assembling dist/windows-x64/"
    VERBATIM
)

set(HP2_WINDOWS_ZIP "${HP2_WINDOWS_DIST_DIR}/HarryPotter2-${PROJECT_VERSION}-windows-x64.zip")
set(HP2_WINDOWS_ZIP_STAGE
    "${CMAKE_CURRENT_BINARY_DIR}/Packaging/HarryPotter2-${PROJECT_VERSION}-windows-x64")
file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/Packaging")
add_custom_target(hp2_windows_zip
    COMMAND "${CMAKE_COMMAND}" -E rm -rf "${HP2_WINDOWS_ZIP_STAGE}"
    COMMAND "${CMAKE_COMMAND}" -E make_directory "${CMAKE_CURRENT_BINARY_DIR}/Packaging"
    COMMAND "${CMAKE_COMMAND}" -E copy_directory "${HP2_WINDOWS_DIST_DIR}" "${HP2_WINDOWS_ZIP_STAGE}"
    COMMAND "${CMAKE_COMMAND}" -E tar cf "${HP2_WINDOWS_ZIP}" --format=zip
        "HarryPotter2-${PROJECT_VERSION}-windows-x64"
    WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/Packaging"
    DEPENDS hp2_windows_dist
    COMMENT "Creating ${HP2_WINDOWS_ZIP}"
    VERBATIM
)
endif()
