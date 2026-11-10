if(NOT DEFINED HP2_PYTHON OR NOT DEFINED HP2_SOURCE_DIR
        OR NOT DEFINED HP2_ORACLE_APP OR NOT DEFINED HP2_ENGINE_BIN
        OR NOT DEFINED HP2_DATA_ROOT OR NOT DEFINED HP2_OUTPUT_DIR)
    message(FATAL_ERROR "parity_paired_maps requires configured Python, source, app, engine, data, and output paths")
endif()

execute_process(
    COMMAND "${HP2_PYTHON}" "${HP2_SOURCE_DIR}/Build/oracle_preflight.py"
    RESULT_VARIABLE _oracle_preflight_result
)
if(NOT _oracle_preflight_result STREQUAL "0")
    message(FATAL_ERROR "native oracle preflight blocked parity: ${_oracle_preflight_result}")
endif()

file(MAKE_DIRECTORY "${HP2_OUTPUT_DIR}")
set(_smoke_report "${HP2_OUTPUT_DIR}/oracle-smoke.json")
set(_parity_report "${HP2_OUTPUT_DIR}/parity.json")
execute_process(
    COMMAND "${HP2_PYTHON}" "${HP2_SOURCE_DIR}/Build/smoke_maps.py"
        --renderer=wgpu
        "--app=${HP2_ORACLE_APP}"
        "--output=${_smoke_report}"
        "--data-root=${HP2_DATA_ROOT}"
    WORKING_DIRECTORY "${HP2_SOURCE_DIR}"
    RESULT_VARIABLE _smoke_result
)
if(NOT _smoke_result EQUAL 0)
    message(FATAL_ERROR "oracle smoke_maps.py failed with exit status ${_smoke_result}; see ${_smoke_report}")
endif()

execute_process(
    COMMAND "${HP2_PYTHON}" "${HP2_SOURCE_DIR}/Build/parity_sweep.py"
        "--oracle-app=${HP2_ORACLE_APP}"
        "--engine-bin=${HP2_ENGINE_BIN}"
        "--data-root=${HP2_DATA_ROOT}"
        "--smoke-report=${_smoke_report}"
        "--output=${_parity_report}"
        --ticks=300
        --fixed-dt=0.02
        --rng-seed=1
        --timeout=180
    WORKING_DIRECTORY "${HP2_SOURCE_DIR}"
    RESULT_VARIABLE _parity_result
)
if(NOT _parity_result EQUAL 0)
    message(FATAL_ERROR "parity_sweep.py failed with exit status ${_parity_result}; see ${_parity_report}")
endif()
