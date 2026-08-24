//! M2 — the object arena: one flat vector of plain objects addressed by
//! [`ObjectId`] handles. No traits, registries, or ECS indirection.
//!
//! Every object carries `{ class_id, name, outer, property_data }` plus a
//! payload discriminating script metadata (classes, functions, states,
//! property templates) from ordinary instances.

use crate::error::{Fail, Result};
use crate::name::{Name, NamePool};
use crate::props::PropStore;
use crate::value::PropertyKind;
use std::collections::HashMap;

/// Stable handle into [`ObjectArena`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub u32);

/// Sibling/super links every serialized field carries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FieldLinks {
    pub super_field: Option<ObjectId>,
    pub next: Option<ObjectId>,
}

/// Per-class script metadata parsed from a Class export.
#[derive(Debug, Clone, Default)]
pub struct ClassData {
    pub links: FieldLinks,
    /// Superclass reference (may be `None` only for Core's Object).
    pub super_class: Option<ObjectId>,
    /// Direct children (fields, functions, states) in serialization order.
    pub children: Vec<ObjectId>,
    pub class_flags: u32,
    /// `ClassWithin` outer constraint.
    pub within: Option<ObjectId>,
    /// Config section name (pool index).
    pub config_name: u32,
    /// Synthesized default-object instance for this class.
    pub default_object: Option<ObjectId>,
}

/// Per-function script metadata.
#[derive(Debug, Clone, Default)]
pub struct FunctionData {
    pub links: FieldLinks,
    pub native_index: u16,
    pub operator_precedence: u8,
    pub function_flags: u32,
    /// Compiled bytecode (token stream).
    pub code: Vec<u8>,
    /// Parameters in declaration order (property-template objects).
    pub params: Vec<ObjectId>,
    /// Locals after the parameter block (declaration order).
    pub locals: Vec<ObjectId>,
}

/// Per-state script metadata.
#[derive(Debug, Clone, Default)]
pub struct StateData {
    pub links: FieldLinks,
    pub probe_mask: u64,
    pub ignore_mask: u64,
    pub state_flags: u32,
    pub label_table_offset: u16,
    pub code: Vec<u8>,
}

/// Property template metadata.
#[derive(Debug, Clone)]
pub struct PropertyData {
    pub links: FieldLinks,
    pub kind: PropertyKind,
    pub array_dim: i32,
    pub property_flags: u32,
    /// Category name (pool index).
    pub category: u32,
}

/// A script struct definition (`UScriptStruct`).
#[derive(Debug, Clone, Default)]
pub struct StructData {
    pub links: FieldLinks,
    pub super_struct: Option<ObjectId>,
    pub children: Vec<ObjectId>,
}

/// What an object holds beyond the common header.
#[derive(Debug, Clone, Default)]
pub enum ObjectData {
    #[default]
    Empty,
    /// Ordinary instance storage (including class default objects).
    Properties(PropStore),
    Class(ClassData),
    Function(Box<FunctionData>),
    State(Box<StateData>),
    Property(Box<PropertyData>),
    ScriptStruct(StructData),
    /// Enum member names (pool indices).
    Enum {
        links: FieldLinks,
        names: Vec<u32>,
    },
    Const {
        links: FieldLinks,
        value: String,
    },
    TextBuffer {
        text: String,
    },
    /// Uninterpreted payload retained verbatim (textures, fonts, sounds…).
    Raw(Vec<u8>),
}

/// One arena slot: the common object header plus its payload.
#[derive(Debug, Clone, Default)]
pub struct UObject {
    pub class_id: Option<ObjectId>,
    /// Canonical base-name index in the shared pool.
    pub name_index: u32,
    pub outer: Option<ObjectId>,
    /// Serialized object flags (`RF_*`).
    pub flags: u32,
    pub data: ObjectData,
}

impl UObject {
    /// Property bag accessor; panics-free variant of expecting instances.
    pub fn properties(&self) -> Option<&PropStore> {
        match &self.data {
            ObjectData::Properties(store) => Some(store),
            _ => None,
        }
    }

    pub fn properties_mut(&mut self) -> Option<&mut PropStore> {
        match &mut self.data {
            ObjectData::Properties(store) => Some(store),
            _ => None,
        }
    }
}

/// The whole object space plus its shared name pool.
#[derive(Debug, Default)]
pub struct ObjectArena {
    pub names: NamePool,
    objects: Vec<UObject>,
    /// Package-root lookup by lowercased package name.
    roots: HashMap<String, ObjectId>,
}

impl ObjectArena {
    pub fn new() -> Self {
        Self::default()
    }

    /// Iterate every object with its id, in allocation order.
    pub fn objects_iter(&self) -> impl Iterator<Item = (ObjectId, &UObject)> {
        self.objects
            .iter()
            .enumerate()
            .map(|(i, o)| (ObjectId(i as u32), o))
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn contains(&self, id: ObjectId) -> bool {
        (id.0 as usize) < self.objects.len()
    }

    pub fn get(&self, id: ObjectId) -> Result<&UObject> {
        self.objects.get(id.0 as usize).ok_or_else(|| {
            Fail::new(
                "script.object_out_of_range",
                format!(
                    "object id {id:?} outside arena of {} objects",
                    self.objects.len()
                ),
            )
        })
    }

    pub fn get_mut(&mut self, id: ObjectId) -> Result<&mut UObject> {
        let len = self.objects.len();
        self.objects.get_mut(id.0 as usize).ok_or_else(|| {
            Fail::new(
                "script.object_out_of_range",
                format!("object id {id:?} outside arena of {len} objects"),
            )
        })
    }

    /// Allocate an empty object; id assignment is append-only and never
    /// reused, keeping handles stable.
    pub fn alloc(&mut self, object: UObject) -> ObjectId {
        let id = ObjectId(self.objects.len() as u32);
        self.objects.push(object);
        id
    }

    /// Register (or find) the package-root object for `name`.
    pub fn root_for(&mut self, name: &str) -> ObjectId {
        let key = name.to_ascii_lowercase();
        if let Some(id) = self.roots.get(&key) {
            return *id;
        }
        let name_index = self.names.intern(name);
        let id = self.alloc(UObject {
            class_id: None,
            name_index,
            outer: None,
            flags: 0,
            data: ObjectData::Properties(PropStore::default()),
        });
        self.roots.insert(key, id);
        id
    }

    /// Find a package root by name.
    pub fn root_lookup(&self, name: &str) -> Option<ObjectId> {
        self.roots.get(&name.to_ascii_lowercase()).copied()
    }

    /// Full dotted path of an object (`Outer.Outer.Name`), canonical spellings.
    pub fn path_of(&self, id: ObjectId) -> Result<String> {
        let mut parts = Vec::new();
        let mut cur = Some(id);
        let mut guard = 0;
        while let Some(next) = cur {
            guard += 1;
            if guard > 100_000 {
                return Err(Fail::new("bind.outer_cycle", format!("at {next:?}")));
            }
            let object = self.get(next)?;
            parts.push(
                self.names
                    .text(object.name_index)
                    .unwrap_or("?")
                    .to_string(),
            );
            cur = object.outer;
        }
        parts.reverse();
        Ok(parts.join("."))
    }

    /// Resolve a dotted path (`Package.Class.Field`) case-insensitively.
    pub fn find_by_path(&self, path: &str) -> Option<ObjectId> {
        let mut parts = path.split('.');
        let first = parts.next()?;
        let mut cur = self.root_lookup(first)?;
        for part in parts {
            cur = self.find_child(cur, part)?;
        }
        Some(cur)
    }

    /// Follow a field sibling chain (`Next` links) from a head object.
    pub fn chain_children(&self, head: Option<ObjectId>) -> Result<Vec<ObjectId>> {
        let mut out = Vec::new();
        let mut cur = head;
        let mut guard = 0;
        while let Some(id) = cur {
            guard += 1;
            if guard > 100_000 {
                return Err(Fail::new("bind.field_chain_cycle", format!("at {id:?}")));
            }
            out.push(id);
            let next = match &self.get(id)?.data {
                ObjectData::Class(d) => d.links.next,
                ObjectData::Function(d) => d.links.next,
                ObjectData::State(d) => d.links.next,
                ObjectData::Property(d) => d.links.next,
                ObjectData::ScriptStruct(d) => d.links.next,
                ObjectData::Enum { links, .. } => links.next,
                ObjectData::Const { links, .. } => links.next,
                _ => None,
            };
            cur = next;
        }
        Ok(out)
    }

    /// Find a function visible on `class_id`, walking up the inheritance
    /// chain and matching case-insensitively.
    pub fn find_function(&self, class_id: ObjectId, name: &str) -> Result<Option<ObjectId>> {
        let target = crate::name::fold_key(name);
        for class in self.class_chain(class_id)? {
            for (id, object) in self.children_of(class) {
                if !matches!(object.data, ObjectData::Function(_)) {
                    continue;
                }
                if self
                    .names
                    .text(object.name_index)
                    .map(|t| crate::name::fold_key(t) == target)
                    .unwrap_or(false)
                {
                    return Ok(Some(id));
                }
            }
        }
        Ok(None)
    }

    /// Case-insensitive child lookup by base name.
    pub fn find_child(&self, parent: ObjectId, name: &str) -> Option<ObjectId> {
        let target = crate::name::fold_key(name);
        self.objects
            .iter()
            .enumerate()
            .find(|(_, o)| {
                o.outer == Some(parent)
                    && self
                        .names
                        .text(o.name_index)
                        .map(|t| crate::name::fold_key(t) == target)
                        .unwrap_or(false)
            })
            .map(|(i, _)| ObjectId(i as u32))
    }

    /// Iterate direct children in arena (allocation) order.
    pub fn children_of<'a>(
        &'a self,
        parent: ObjectId,
    ) -> impl Iterator<Item = (ObjectId, &'a UObject)> + 'a {
        self.objects
            .iter()
            .enumerate()
            .filter(move |(_, o)| o.outer == Some(parent))
            .map(|(i, o)| (ObjectId(i as u32), o))
    }

    /// Walk a class's inheritance chain (self first).
    pub fn class_chain(&self, class_id: ObjectId) -> Result<Vec<ObjectId>> {
        let mut chain = Vec::new();
        let mut cur = Some(class_id);
        let mut guard = 0;
        while let Some(id) = cur {
            guard += 1;
            if guard > 4096 {
                return Err(Fail::new(
                    "bind.class_cycle",
                    format!("inheritance cycle reached at {id:?}"),
                ));
            }
            chain.push(id);
            cur = match &self.get(id)?.data {
                ObjectData::Class(data) => data.super_class,
                ObjectData::ScriptStruct(data) => data.super_struct,
                _ => None,
            };
        }
        Ok(chain)
    }

    /// True when `class_id` equals or inherits from `base_id`.
    pub fn class_is_a(&self, class_id: ObjectId, base_id: ObjectId) -> Result<bool> {
        Ok(self.class_chain(class_id)?.contains(&base_id))
    }

    /// Dotted display name of an object reference.
    pub fn display_name(&self, id: ObjectId) -> Result<String> {
        let object = self.get(id)?;
        Ok(self
            .names
            .text(object.name_index)
            .unwrap_or("?")
            .to_string())
    }

    /// Build a `Name` handle for a pool index.
    pub fn name_handle(&self, index: u32) -> Name {
        Name {
            index,
            number: crate::name::NO_NUMBER,
        }
    }
}
