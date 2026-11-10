# Opt in per build tree. Cargo manages fingerprints and locks within this cache.
option(HP2_SHARED_TEST_CARGO_CACHE "Share a Cargo target directory among behavior tests" OFF)

# Preserve the command except for CARGO_TARGET_DIR assignments in the leading
# environment of a CMake -E env command with an executable following it.
function(hp2_prepare_cargo_test_command out_var)
    set(_rewrite FALSE)
    if(HP2_SHARED_TEST_CARGO_CACHE AND ARGC GREATER 4
       AND "${ARGV1}" STREQUAL "${CMAKE_COMMAND}"
       AND "${ARGV2}" STREQUAL "-E" AND "${ARGV3}" STREQUAL "env")
        set(_index 4)
        while(_index LESS ARGC)
            set(_arg "${ARGV${_index}}")
            if(NOT _arg MATCHES "^[^=]+=")
                if(NOT _arg STREQUAL "")
                    set(_rewrite TRUE)
                endif()
                break()
            endif()
            math(EXPR _index "${_index} + 1")
        endwhile()
    endif()

    set(_result "")
    set(_in_env ${_rewrite})
    if(ARGC GREATER 1)
        math(EXPR _last "${ARGC} - 1")
        foreach(_index RANGE 1 ${_last})
            set(_arg "${ARGV${_index}}")
            if(_in_env AND _index GREATER_EQUAL 4)
                if(NOT _arg MATCHES "^[^=]+=")
                    set(_in_env FALSE)
                elseif(_arg MATCHES "^CARGO_TARGET_DIR=")
                    set(_arg "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/Testing/HP2/CargoCache")
                endif()
            endif()
            # CMake lists use semicolons as separators, including in values.
            string(REPLACE ";" "\\;" _arg "${_arg}")
            list(APPEND _result "${_arg}")
        endforeach()
    endif()
    set(${out_var} "${_result}" PARENT_SCOPE)
endfunction()
