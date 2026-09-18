# LifecycleGate staging

The lifecycle gate never invokes UCC or alters `Default.ini`.

1. Commit the native `Package/LifecycleGate.u` artifact and record its exact lowercase SHA-256 in `manifest.json` under `package_artifact.sha256`, alongside non-empty provenance metadata.
2. The harness verifies the package artifact before staging. It then copies those package bytes to `System/LifecycleGate.u` in both isolated roots.
3. It copies the base data root's `Maps/Entry.unr` byte-for-byte to `Maps/LifecycleGate.unr` in both roots. The fixture is selected only through `?game=LifecycleGate.LifecycleGateGame`.

The checked-in manifest names the required native package and its provenance but has no digest while that artifact is absent. The gate therefore stops with an artifact/provenance diagnostic; it does not compile, synthesize, or substitute a package.

`Classes/` and `Maps/LifecycleGate.t3d` remain the authored sources; the T3D is reserved for the later map-loader gate and is not used here.

Launch `Maps/LifecycleGate.unr?game=LifecycleGate.LifecycleGateGame` with `-TESTTICKS=2 -fixed-dt=0.016666667` and set `HP2_LIFECYCLE_GATE_TRACE` to the output JSON path.
