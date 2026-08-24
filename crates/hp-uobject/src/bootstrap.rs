//! Bootstrap: load the stock packages in dependency order, bind natives,
//! and hand back a ready-to-run [`World`].

use std::collections::HashMap;
use std::path::Path;

use crate::arena::{ObjectArena, ObjectId};
use crate::error::{Fail, Result};
use crate::natives::NativeRegistry;

/// Load order mirrors the engine: parents before children. `HPModels`
/// precedes `HGame` because retail `HGame` classes (e.g. `HGame.QuidArmor`)
/// declare their superclass in `HPModels`, and import stitching only
/// resolves against already-loaded packages.
pub const PACKAGE_LOAD_ORDER: [&str; 6] = [
    "Core",
    "Engine",
    "UWindow",
    "HPParticle",
    "HPModels",
    "HGame",
];

/// A fully loaded and bound object space plus its native dispatch table.
pub struct World {
    pub arena: ObjectArena,
    pub registry: NativeRegistry,
    /// Package name → package-root object id.
    pub roots: HashMap<String, ObjectId>,
    /// Package name → every export id (bind's untyped-export audit set).
    pub exports: HashMap<String, Vec<ObjectId>>,
}

impl World {
    /// Load every package under `<root>/System/<name>.u`, bind all classes
    /// and natives. Any gap is a loud error — bootstrap never half-loads.
    pub fn load(data_root: &Path) -> Result<World> {
        let mut arena = ObjectArena::new();
        let mut roots = HashMap::new();
        let mut exports: HashMap<String, Vec<ObjectId>> = HashMap::new();
        for name in PACKAGE_LOAD_ORDER {
            let path = data_root.join("System").join(format!("{name}.u"));
            let bytes = std::fs::read(&path).map_err(|e| {
                Fail::new(
                    "bootstrap.package_unreadable",
                    format!("{}: {e}", path.display()),
                )
            })?;
            let loaded = crate::loader::load_package(&mut arena, name, &bytes)?;
            roots.insert(name.to_string(), loaded.root);
            exports.insert(name.to_string(), loaded.export_ids);
        }

        let mut registry = NativeRegistry::new();
        let mut all_exports = Vec::new();
        for ids in exports.values() {
            all_exports.extend(ids.iter().copied());
        }
        crate::bind::bind_world(&arena, &mut registry, &all_exports)?;

        Ok(World {
            arena,
            registry,
            roots,
            exports,
        })
    }
}
