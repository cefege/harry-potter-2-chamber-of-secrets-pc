# Copies the Emscripten module payload produced by the hp2_game link into the
# served directory. Run in script mode: cmake -DBUILD_DIR=<dir> -DOUTPUT_DIR=<dir>
# -P HP2WebStage.cmake
#
# The glob covers hp2_game.js, hp2_game.wasm, and any auxiliary module the
# toolchain emits for the same target (for example hp2_game.worker.js when
# Emscripten moves pthreads to a worker).

foreach(_hp2_stage_arg IN ITEMS BUILD_DIR OUTPUT_DIR)
    if(NOT DEFINED ${_hp2_stage_arg} OR "${${_hp2_stage_arg}}" STREQUAL "")
        message(FATAL_ERROR
            "HP2WebStage.cmake requires -D${_hp2_stage_arg}=<path>")
    endif()
endforeach()

file(GLOB _hp2_stage_payload "${BUILD_DIR}/hp2_game.*")
set(_hp2_stage_selected "")
foreach(_hp2_stage_file IN LISTS _hp2_stage_payload)
    if(_hp2_stage_file MATCHES "\\.(js|wasm)$")
        list(APPEND _hp2_stage_selected "${_hp2_stage_file}")
    endif()
endforeach()
list(LENGTH _hp2_stage_selected _hp2_stage_count)
if(_hp2_stage_count EQUAL 0)
    message(FATAL_ERROR "No hp2_game.* js/wasm payload found in ${BUILD_DIR}")
endif()

file(COPY ${_hp2_stage_selected} DESTINATION "${OUTPUT_DIR}")
message(STATUS "Staged ${_hp2_stage_count} module file(s) into ${OUTPUT_DIR}")
