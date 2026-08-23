# Native Text Providers

The native text backend is built through an explicit **provider model** in
`Build/CMake/HP2Dependencies.cmake`. Each platform backend is an ordered
candidate; selection picks the first whose condition passes at configure time
and exports its inputs as variables. No caller hardcodes provider paths.

## How it works

1. `hp2_declare_text_provider(NAME SOURCES... LINK_LIBS... CONDITION)` records
   a candidate in declaration order (global properties).
2. Selection iterates candidates, evaluates each `CONDITION`, and for the
   first match sets:
   - `HP2_HAS_NATIVE_TEXT_BACKEND` — `ON` when a provider was selected,
   - `HP2_TEXT_PROVIDER_NAME` — the selected provider name (`""` if none),
   - `HP2_TEXT_PROVIDER_SOURCES` — the provider's translation units,
   - `HP2_TEXT_PROVIDER_LINK_LIBS` — libraries the owning driver links `PUBLIC`.
3. Consumers read only these variables:
   - `Build/CMake/HP2Dependencies.cmake` appends `HP2_TEXT_PROVIDER_SOURCES`
     to `HP2_XOPENGLDRV_SOURCES`.
   - `Build/CMake/HP2Targets.cmake` links `HP2_TEXT_PROVIDER_LINK_LIBS` into
     `hp2_xopengldrv` and gates test targets/registrations on
     `HP2_HAS_NATIVE_TEXT_BACKEND`.

Today there is exactly one provider:

| Name | Condition | Sources | Link libs |
|---|---|---|---|
| `CoreText` | `APPLE AND HP2_CORETEXT_FRAMEWORK AND HP2_COREGRAPHICS_FRAMEWORK` | `ThirdParty/XOpenGLDrv/Src/NativeText.cpp` | CoreText, CoreGraphics |

The framework probes run unconditionally before the declarations so every
condition is a pure variable test; on hosts without the frameworks
`find_library` simply reports `NOTFOUND`. Conditions are evaluated with
`cmake_language(EVAL CODE ...)` because a plain `if(${condition})` does not
re-dereference expanded non-boolean tokens such as framework paths.

When no provider matches, configure succeeds with
`HP2_HAS_NATIVE_TEXT_BACKEND=OFF`; the text-dependent test executables and
registrations simply do not exist (a configuration gap, never a silent skip).

## Adding a provider (DirectWrite or Pango example)

Add one declare call after the corresponding probes; nothing else changes.
Order matters: earlier declarations win, so put the preferred backend first.

```cmake
# Windows: DirectWrite
find_library(HP2_DWRITE_FRAMEWORK DirectWrite)          # or find_package / pkg-config for Pango
hp2_declare_text_provider(DirectWrite
    SOURCES "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/NativeTextDirectWrite.cpp"
    LINK_LIBS "${HP2_DWRITE_FRAMEWORK}"
    CONDITION "WIN32 AND HP2_DWRITE_FRAMEWORK"
)
```

For Pango on Linux, probe with `pkg_check_modules` (or `find_package(Pango)`)
and use a condition like `PANGO_FOUND AND CAIRO_FOUND`. The new provider's
sources must expose the same `FNativeTextPlatformBackend` seam that
`ThirdParty/XOpenGLDrv/Src/NativeText.cpp` implements; the driver and tests
compile against the interface, not the backend.

## Gate interaction: capability is not default-enabled

Selection answers "can this graph compile a native text backend?" It says
nothing about whether the feature runs by default. That decision lives in
`Build/feature-gates.json` (`native_text`: state `experimental`,
`default_enabled: false`): even with a provider selected, the runtime stays on
the original bitmap-font path unless `[Display] NativeText=True` is set
explicitly (launcher toggle or config). Promotion to default-enabled requires
the evidence listed in the gate file. Keep the two decisions separate:
adding a provider widens capability; touching the gate changes behavior.
