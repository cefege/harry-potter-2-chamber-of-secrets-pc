# Browser (WebAssembly) build surface. Included only under EMSCRIPTEN by the
# root CMakeLists; the native path includes HP2Install instead.
#
# Milestone 1 ships a single module plus a static shell, served from one
# directory with COOP/COEP so pthreads stay real threads. Every claim about this
# port holds for the data-prototype profile only.

set(HP2_WEB_DIST_DIR "${PROJECT_SOURCE_DIR}/dist/web-wasm64")
set(HP2_WEB_SHELL_DIR "${PROJECT_SOURCE_DIR}/Web")

if(NOT TARGET hp2_game)
    message(FATAL_ERROR "HP2Web requires the hp2_game target")
endif()
if(NOT EXISTS "${HP2_WEB_SHELL_DIR}/index.html"
        OR NOT EXISTS "${HP2_WEB_SHELL_DIR}/hp2-web.js")
    message(FATAL_ERROR
        "HP2Web requires Web/index.html and Web/hp2-web.js to stage the served page")
endif()

# Emscripten defaults the executable suffix to .js; state it so the target file
# name (hp2_game.js + hp2_game.wasm) does not depend on toolchain defaults.
set_target_properties(hp2_game PROPERTIES SUFFIX ".js")

# -sMEMORY64=1 -pthread -fwasm-exceptions -sSUPPORT_LONGJMP=wasm already come
# from the preset's global flags so FetchContent dependencies compile for the
# same target.
target_link_options(hp2_game PRIVATE
    -sMODULARIZE=1
    -sEXPORT_NAME=createHP2Module
    -sENVIRONMENT=web,worker
    # main() hands the loop to requestAnimationFrame and returns, so the runtime
    # must stay alive after it returns.
    -sEXIT_RUNTIME=0
    # The module is instantiated when the page loads, not when the player
    # presses a button, so the runtime must not run main on its own: the
    # shell decides the map and save from its own UI.
    -sINVOKE_RUN=0
    -sINITIAL_MEMORY=536870912
    -sMAXIMUM_MEMORY=4294967296
    -sSTACK_SIZE=8388608
    -sDEFAULT_PTHREAD_STACK_SIZE=2097152
    -sPTHREAD_POOL_SIZE=4
    # The engine opens packages through the FS API; the browser shell copies
    # imported data into MEMFS and mounts IDBFS over the user root.
    -sFORCE_FILESYSTEM=1
    -lidbfs.js
    # AL is exported so the shell can inspect the device/context state and
    # resume contexts the autoplay policy suspended. callMain is the launch
    # entry point: it runs main with the launcher's arguments.
    "-sEXPORTED_RUNTIME_METHODS=FS,ENV,AL,addRunDependency,removeRunDependency,callMain"
)

add_custom_target(hp2_web ALL
    DEPENDS hp2_game
    COMMAND "${CMAKE_COMMAND}" -E rm -rf "${HP2_WEB_DIST_DIR}"
    COMMAND "${CMAKE_COMMAND}" -E make_directory "${HP2_WEB_DIST_DIR}"
    COMMAND "${CMAKE_COMMAND}"
            "-DBUILD_DIR=$<TARGET_FILE_DIR:hp2_game>"
            "-DOUTPUT_DIR=${HP2_WEB_DIST_DIR}"
            -P "${CMAKE_CURRENT_LIST_DIR}/HP2WebStage.cmake"
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "${HP2_WEB_SHELL_DIR}/index.html" "${HP2_WEB_DIST_DIR}/index.html"
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "${HP2_WEB_SHELL_DIR}/hp2-web.js" "${HP2_WEB_DIST_DIR}/hp2-web.js"
    COMMENT "Staging the WebAssembly browser build in ${HP2_WEB_DIST_DIR}"
    VERBATIM
)

# Headless verification for the browser build. Both drive the real staged
# directory in a real browser: shell proves cross-origin isolation plus
# wasm64 module instantiation (data-none), boot proves the engine reaches its
# main loop and renders (data-prototype, so it is gated on the data root).
hp2_add_behavior_test(web_shell_smoke "${Python3_EXECUTABLE}"
    "${PROJECT_SOURCE_DIR}/Build/web_smoke.py" --mode=shell "--root=${HP2_WEB_DIST_DIR}")
set_tests_properties(web_shell_smoke PROPERTIES LABELS "smoke;data-none")
if(HP2_TEST_DATA_PRESENT)
    hp2_add_behavior_test(web_boot_smoke "${Python3_EXECUTABLE}"
        "${PROJECT_SOURCE_DIR}/Build/web_smoke.py" --mode=boot "--root=${HP2_WEB_DIST_DIR}"
        "--data-root=${HP2_TEST_DATA_ROOT}" --map=PrivetDr.unr)
    set_tests_properties(web_boot_smoke PROPERTIES
        LABELS "smoke;data-prototype" RESOURCE_LOCK hp2_gpu)
endif()
