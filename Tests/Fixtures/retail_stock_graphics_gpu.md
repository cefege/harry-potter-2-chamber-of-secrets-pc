# Retail advanced graphics replay protocol

Status: prepared, **not calibrated or run**. Wait until the current display owner releases the display. Use a private build and a disposable retail data copy verified by a TSV size/SHA-256 inventory. Do not reuse a personal game-data tree. Do not infer retail authenticity from that inventory.

`Tests/Fixtures/retail_stock_advanced_graphics_open.txt` opens the stock page for a calibration screenshot only. `Tests/RetailStockAdvancedGraphicsSmoke.py` accepts 2 caller-supplied interaction fixtures. It does not assume mouse coordinates. Calibrate them against captured frames at 800×600 on the target build. The first fixture must open the original stock sound/video page, select each setting in the Advanced selector, and change these initial values:

| Control | Initial | Target |
|---|---:|---:|
| Frame limit | 60 | 120 |
| UI scale | 100% | 125% |
| Vertical FOV | On | Off |
| Native text | On | Off |
| Screen flashes | On | Off |
| VSync | opposite of `--vsync` | `--vsync` |
| Anisotropy | 0, or 4 when target is 0 | `--anisotropy` |
| Anti-aliasing | Off, or 4x when target is 0 | `--msaa` |

Choose GPU values that the actual surface and adapter support. Do not claim the opposite VSync mode is supported merely because it is a reflected value. The second fixture reopens the same page after a fresh process starts and selects each option to capture its persisted value. Recalibrate coordinates after changing UI scale: the interface geometry can change immediately.

Capture checkpoints that show the stock page before interaction, the Advanced selector expanded, the value selector expanded, all changed settings, and the restart values. Include at least one screenshot at each UI scale. Inspect the images for discoverability, clipping, overlapping stock controls, legible labels, focus, and dropdown placement. The script reports `needs-visual-review` even when automated checks pass. Record the review separately; existence of PNG files is not visual acceptance.

The replay requires the engine to emit, with `HP2_STOCK_GRAPHICS_GPU_TRACE=1`, actual live-state probes:

```
StockGraphicsProbe event=applied setting=frame_limit requested=120 actual=120
StockGraphicsProbe event=created setting=frame_limit requested=120 actual=120
StockGraphicsGPU event=applied setting=vsync requested=on actual=on
StockGraphicsGPU event=created setting=vsync requested=on actual=on
```

Analogous probes are required for `ui_scale` (integer percent), `vertical_fov`, `native_text`, `screen_flashes` (`on`/`off`), `anisotropy` (`0`/`4`/`8`/`16`), and `msaa` (`0`/`2`/`4`). `created` must reflect the effective value after restart, not a copy of the requested or persisted setting. GPU probes must reflect the surface present mode and effective sampler clamp. Missing probes fail the run. The source includes opt-in probes, but their retail runtime behavior is not yet verified.

Run only after build, input calibration, trace implementation, and display release:

```sh
python3 Tests/RetailStockAdvancedGraphicsSmoke.py \
  --data-root /path/to/disposable/retail --inventory /path/to/size-sha256.tsv \
  --engine-bin /path/to/private/engine --engine-sha256 <sha256> \
  --set-fixture /path/to/calibrated-set.txt --restart-fixture /path/to/calibrated-restart.txt \
  --set-checkpoints <comma-separated-ticks> --restart-checkpoints <comma-separated-ticks> \
  --vsync on --anisotropy 4 --msaa 4 --artifact-dir /path/to/empty/artifacts --allow-window
```

The runner checks the engine hash before and after each phase, the retail inventory before and after, a distinct starting effective value followed by a live-apply probe for each setting, restart readback probes, saved INI values, failures, and checkpoint existence. A persistence or GPU error must not be hidden by retries. Explicitly report unsupported hardware, missing screenshot review, physical-input limitations, and untested platforms.
