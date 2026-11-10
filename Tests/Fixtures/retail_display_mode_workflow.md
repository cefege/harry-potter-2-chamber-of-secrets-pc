# Retail display-mode replay fixtures

`Tests/RetailDisplayModeSmoke.py` uses `retail_display_mode_open.txt` for its
initial phase. Provide a separate, reviewed fixture directory for the remaining
phases, including restart. Do not assume that fullscreen coordinates equal windowed
coordinates. First capture the page and dropdown on the target display.

Each file uses the `InputScriptSmoke.py` DSL, starts with a `tick` directive,
opens the original sound/video page, and has a `# capture: 123,456` annotation.
Use frame indices from the resulting replay. Capture the page, the dropdown,
the confirmation, and the final state. A capture is not a visual approval.

- `keep.txt`: select **Borderless Fullscreen**, then choose **Yes**.
- `restart.txt`: reopen the graphics page after the saved borderless restart.
  Capture the page and the reported live mode.
- `revert.txt`: from saved borderless mode, select **Windowed**, then choose
  **No**.
- `timeout.txt`: from saved borderless mode, select **Windowed** and send no
  further event, including mouse movement, for at least 21 seconds of replay
  time. Capture the confirmation and the restored page.
- `exclusive.txt`: from saved borderless mode, select **Fullscreen
  (exclusive)**. If SDL accepts it, choose **No**. If SDL rejects it,
  capture the error and do not issue a confirmation input.

The harness rejects a timeout fixture without the required event-free tail.
Do not replace a failed run's fixtures and report only the later run. Keep each
run's artifacts and report every failure. Synthetic input is not physical-input
evidence, and SDL's exclusive flag is not independent OS-level proof.

The full workflow requires `--foreground`; it must not compete with another
agent's fullscreen test. Each phase requires a nonzero process-wide WGPU
presentation tally. That tally does not prove which mode presented the frames.

The harness checks the executable hash before and during the run, checks the
retail inventory before and after, uses an isolated HOME, stops after a failed
phase, and records `Game.ini`, logs, captures, and `report.json` under the
artifact directory. Use a disposable retail copy and an inventory with relative
path, byte count, and SHA-256 columns separated by tabs.
