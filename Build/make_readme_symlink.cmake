# Place a "drag to install" README symlink next to an app bundle.
#
# invocation:
#   cmake -P make_readme_symlink.cmake <parent_dir> <app_dir>
#
# Both arguments are absolute paths: <app_dir> is a directory ending in
# ".app" and <parent_dir> is the directory containing it. The link is named
# "README - drag to install.txt" and points at the app, which is the
# conventional drag-to-Applications affordance in a disk image.
#
# Idempotent, and never fatal: the link is a convenience, so an
# already-correct link is left alone and any genuine problem is reported as a
# WARNING rather than failing the surrounding build.

# CMake 4 removed the script-mode ARGV/ARGC variables; the positional form is
# the only portable spelling, and CMAKE_ARGV3/4 are the first two user
# arguments because 0..2 are "cmake", "-P", and this script.
if(NOT DEFINED CMAKE_ARGV3 OR NOT DEFINED CMAKE_ARGV4)
    message(FATAL_ERROR "make_readme_symlink.cmake requires <parent_dir> <app_dir>")
endif()

set(parent_arg "${CMAKE_ARGV3}")
set(app_arg "${CMAKE_ARGV4}")

get_filename_component(parent "${parent_arg}" REALPATH)
get_filename_component(app "${app_arg}" REALPATH)
get_filename_component(app_name "${app}" NAME)

set(link "${parent}/README - drag to install.txt")

if(IS_SYMLINK "${link}")
    file(READ_SYMLINK "${link}" current)
    get_filename_component(current "${current}" REALPATH)
    if("${current}" STREQUAL "${app}")
        return()
    endif()
elseif(EXISTS "${link}")
    message(WARNING "Refusing to replace non-symlink ${link}")
    return()
endif()

file(REMOVE "${link}")
execute_process(
    COMMAND /bin/ln -s "${app_name}" "${link}"
    RESULT_VARIABLE status
)
if(NOT status EQUAL 0)
    message(WARNING "Could not create ${link} (ln exited ${status})")
endif()
