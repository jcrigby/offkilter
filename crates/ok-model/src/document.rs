//! A document: tabs holding part studios and assemblies.
//!
//! Every edit is a [`DocOp`]: an op on a studio tab, an op on an assembly
//! tab, or a tab-level change. Ids for tabs, instances and mates come from
//! one document counter (collaborative id bases apply to it too); features
//! inside a studio keep that studio's own counter.

use crate::{
    Assembly, AssemblyResult, Connector, Instance, InstanceId, Mate, MateId, MateKind, ModelError,
    Op, OpResult, PartStudio, Placement, RegenResult, TabId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TabKind {
    PartStudio(PartStudio),
    Assembly(Assembly),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub id: TabId,
    pub kind: TabKind,
}

impl Tab {
    pub fn name(&self) -> &str {
        match &self.kind {
            TabKind::PartStudio(p) => &p.name,
            TabKind::Assembly(a) => &a.name,
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match &self.kind {
            TabKind::PartStudio(_) => "part_studio",
            TabKind::Assembly(_) => "assembly",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    pub tabs: Vec<Tab>,
    next_id: u32,
}

/// Edits to a document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocOp {
    AddPartStudio {
        name: Option<String>,
    },
    AddAssembly {
        name: Option<String>,
    },
    RenameTab {
        tab: TabId,
        name: String,
    },
    DeleteTab {
        tab: TabId,
    },
    /// Puts a tab back (the inverse of `DeleteTab`).
    InsertTab {
        index: usize,
        tab: serde_json::Value,
    },
    RenameDocument {
        name: String,
    },
    /// An edit to a part studio tab.
    Studio {
        tab: TabId,
        op: Op,
    },
    /// An edit to an assembly tab.
    Assembly {
        tab: TabId,
        op: AssemblyOp,
    },
    /// Replace the whole document (version restore).
    ReplaceDocument {
        json: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssemblyOp {
    AddInstance {
        studio: TabId,
        body: usize,
        name: Option<String>,
        #[serde(default)]
        fixed: bool,
        #[serde(default)]
        placement: Placement,
    },
    RemoveInstance {
        id: InstanceId,
    },
    SetInstance {
        id: InstanceId,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        fixed: Option<bool>,
        #[serde(default)]
        placement: Option<Placement>,
    },
    AddMate {
        #[serde(default)]
        kind: MateKind,
        a: Connector,
        b: Connector,
        #[serde(default)]
        offset: f64,
        #[serde(default)]
        angle: f64,
        #[serde(default)]
        flip: bool,
        name: Option<String>,
    },
    SetMate {
        id: MateId,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        kind: Option<MateKind>,
        #[serde(default)]
        a: Option<Connector>,
        #[serde(default)]
        b: Option<Connector>,
        #[serde(default)]
        offset: Option<f64>,
        #[serde(default)]
        angle: Option<f64>,
        #[serde(default)]
        flip: Option<bool>,
    },
    RemoveMate {
        id: MateId,
    },
    /// Replaces the instance and mate lists wholesale (undo of structure edits).
    Restore {
        instances: Vec<Instance>,
        mates: Vec<Mate>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocOpResult {
    pub tab: Option<TabId>,
    pub instance: Option<InstanceId>,
    pub mate: Option<MateId>,
    /// Result of a studio op.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub studio: Option<OpResult>,
    /// Ops that undo this one, in the order to apply them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inverse: Vec<DocOp>,
}

impl Default for Document {
    fn default() -> Self {
        Document::new("Document")
    }
}

impl Document {
    /// A document with one empty part studio.
    pub fn new(name: impl Into<String>) -> Document {
        let mut d = Document {
            name: name.into(),
            tabs: Vec::new(),
            next_id: 1,
        };
        d.push_tab(TabKind::PartStudio(PartStudio::new("Part Studio 1")));
        d
    }

    /// A document wrapping one part studio (also how legacy files load).
    pub fn from_studio(studio: PartStudio) -> Document {
        let mut d = Document {
            name: studio.name.clone(),
            tabs: Vec::new(),
            next_id: 1,
        };
        d.push_tab(TabKind::PartStudio(studio));
        d
    }

    pub fn demo() -> Document {
        Document::from_studio(PartStudio::demo())
    }

    /// Parses a document, accepting a bare part studio from older files.
    pub fn from_json(json: &str) -> Result<Document, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        if value.get("tabs").is_some() {
            serde_json::from_value(value)
        } else {
            let studio: PartStudio = serde_json::from_value(value)?;
            Ok(Document::from_studio(studio))
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("document serialises")
    }

    fn alloc(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn push_tab(&mut self, kind: TabKind) -> TabId {
        let id = TabId(self.alloc());
        self.tabs.push(Tab { id, kind });
        id
    }

    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn tab_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn studio(&self, id: TabId) -> Result<&PartStudio, ModelError> {
        match self.tab(id).map(|t| &t.kind) {
            Some(TabKind::PartStudio(p)) => Ok(p),
            _ => Err(ModelError::Invalid(format!(
                "tab {} is not a part studio",
                id.0
            ))),
        }
    }

    pub fn studio_mut(&mut self, id: TabId) -> Result<&mut PartStudio, ModelError> {
        match self.tab_mut(id).map(|t| &mut t.kind) {
            Some(TabKind::PartStudio(p)) => Ok(p),
            _ => Err(ModelError::Invalid(format!(
                "tab {} is not a part studio",
                id.0
            ))),
        }
    }

    pub fn assembly(&self, id: TabId) -> Result<&Assembly, ModelError> {
        match self.tab(id).map(|t| &t.kind) {
            Some(TabKind::Assembly(a)) => Ok(a),
            _ => Err(ModelError::Invalid(format!(
                "tab {} is not an assembly",
                id.0
            ))),
        }
    }

    pub fn assembly_mut(&mut self, id: TabId) -> Result<&mut Assembly, ModelError> {
        match self.tab_mut(id).map(|t| &mut t.kind) {
            Some(TabKind::Assembly(a)) => Ok(a),
            _ => Err(ModelError::Invalid(format!(
                "tab {} is not an assembly",
                id.0
            ))),
        }
    }

    /// The first part studio tab, if any (what a single-studio client edits).
    pub fn first_studio(&self) -> Option<TabId> {
        self.tabs
            .iter()
            .find(|t| matches!(t.kind, TabKind::PartStudio(_)))
            .map(|t| t.id)
    }

    fn numbered(&self, base: &str) -> String {
        let n = self
            .tabs
            .iter()
            .filter(|t| t.name().starts_with(base))
            .count()
            + 1;
        format!("{base} {n}")
    }

    /// Applies an op, allocating any new ids from `base` when given.
    pub fn apply_with_base(
        &mut self,
        op: DocOp,
        base: Option<u32>,
    ) -> Result<DocOpResult, ModelError> {
        if let Some(b) = base {
            self.next_id = b;
        }
        let mut out = DocOpResult::default();
        match op {
            DocOp::AddPartStudio { name } => {
                let name = name.unwrap_or_else(|| self.numbered("Part Studio"));
                out.tab = Some(self.push_tab(TabKind::PartStudio(PartStudio::new(name))));
            }
            DocOp::AddAssembly { name } => {
                let name = name.unwrap_or_else(|| self.numbered("Assembly"));
                out.tab = Some(self.push_tab(TabKind::Assembly(Assembly::new(name))));
            }
            DocOp::RenameTab { tab, name } => {
                let t = self
                    .tab_mut(tab)
                    .ok_or_else(|| ModelError::Invalid(format!("no tab {}", tab.0)))?;
                match &mut t.kind {
                    TabKind::PartStudio(p) => p.name = name,
                    TabKind::Assembly(a) => a.name = name,
                }
            }
            DocOp::DeleteTab { tab } => {
                let pos = self
                    .tabs
                    .iter()
                    .position(|t| t.id == tab)
                    .ok_or_else(|| ModelError::Invalid(format!("no tab {}", tab.0)))?;
                if self.tabs.len() == 1 {
                    return Err(ModelError::Invalid(
                        "a document keeps at least one tab".into(),
                    ));
                }
                self.tabs.remove(pos);
            }
            DocOp::InsertTab { index, tab } => {
                let tab: Tab =
                    serde_json::from_value(tab).map_err(|e| ModelError::Invalid(e.to_string()))?;
                if self.tab(tab.id).is_some() {
                    return Err(ModelError::Invalid(format!(
                        "tab {} already exists",
                        tab.id.0
                    )));
                }
                self.next_id = self.next_id.max(tab.id.0 + 1);
                out.tab = Some(tab.id);
                let index = index.min(self.tabs.len());
                self.tabs.insert(index, tab);
            }
            DocOp::RenameDocument { name } => self.name = name,
            DocOp::Studio { tab, op } => {
                let r = self.studio_mut(tab)?.apply_with_inverse(op, base)?;
                out.tab = Some(tab);
                out.studio = Some(r);
            }
            DocOp::Assembly { tab, op } => {
                out.tab = Some(tab);
                self.apply_assembly(tab, op, &mut out)?;
            }
            DocOp::ReplaceDocument { json } => {
                let d =
                    Document::from_json(&json).map_err(|e| ModelError::Invalid(e.to_string()))?;
                *self = d;
            }
        }
        Ok(out)
    }

    fn apply_assembly(
        &mut self,
        tab: TabId,
        op: AssemblyOp,
        out: &mut DocOpResult,
    ) -> Result<(), ModelError> {
        // Validate references against the document before touching the assembly.
        if let AssemblyOp::AddInstance { studio, .. } = &op {
            if self.tab(*studio).is_none() {
                return Err(ModelError::Invalid(format!("no tab {}", studio.0)));
            }
            if *studio == tab {
                return Err(ModelError::Invalid(
                    "an assembly cannot contain itself".into(),
                ));
            }
        }
        let new_id = |d: &mut Document| d.alloc();
        match op {
            AssemblyOp::AddInstance {
                studio,
                body,
                name,
                fixed,
                placement,
            } => {
                let id = InstanceId(new_id(self));
                let studio_name = self
                    .tab(studio)
                    .map(|t| t.name().to_string())
                    .unwrap_or_default();
                let asm = self.assembly_mut(tab)?;
                let name = name.unwrap_or_else(|| {
                    let n = asm
                        .instances
                        .iter()
                        .filter(|i| i.studio == studio && i.body == body)
                        .count()
                        + 1;
                    format!("{studio_name} <{}>", n)
                });
                asm.instances.push(Instance {
                    id,
                    name,
                    studio,
                    body,
                    fixed,
                    placement,
                });
                out.instance = Some(id);
            }
            AssemblyOp::RemoveInstance { id } => {
                let asm = self.assembly_mut(tab)?;
                let pos = asm
                    .instances
                    .iter()
                    .position(|i| i.id == id)
                    .ok_or_else(|| ModelError::Invalid(format!("no instance {}", id.0)))?;
                asm.instances.remove(pos);
                asm.mates
                    .retain(|m| m.a.instance != id && m.b.instance != id);
            }
            AssemblyOp::SetInstance {
                id,
                name,
                fixed,
                placement,
            } => {
                let asm = self.assembly_mut(tab)?;
                let inst = asm
                    .instances
                    .iter_mut()
                    .find(|i| i.id == id)
                    .ok_or_else(|| ModelError::Invalid(format!("no instance {}", id.0)))?;
                if let Some(n) = name {
                    inst.name = n;
                }
                if let Some(f) = fixed {
                    inst.fixed = f;
                }
                if let Some(p) = placement {
                    if !(p.position.x.is_finite()
                        && p.position.y.is_finite()
                        && p.position.z.is_finite()
                        && p.rotation.x.is_finite()
                        && p.rotation.y.is_finite()
                        && p.rotation.z.is_finite())
                    {
                        return Err(ModelError::Invalid("placement must be finite".into()));
                    }
                    inst.placement = p;
                }
            }
            AssemblyOp::AddMate {
                kind,
                a,
                b,
                offset,
                angle,
                flip,
                name,
            } => {
                if a.instance == b.instance {
                    return Err(ModelError::Invalid(
                        "a mate joins two different instances".into(),
                    ));
                }
                let id = MateId(new_id(self));
                let asm = self.assembly_mut(tab)?;
                for c in [&a, &b] {
                    if asm.instance(c.instance).is_none() {
                        return Err(ModelError::Invalid(format!("no instance {}", c.instance.0)));
                    }
                }
                let name = name.unwrap_or_else(|| format!("Mate {}", asm.mates.len() + 1));
                asm.mates.push(Mate {
                    id,
                    name,
                    kind,
                    a,
                    b,
                    offset,
                    angle,
                    flip,
                });
                out.mate = Some(id);
            }
            AssemblyOp::SetMate {
                id,
                name,
                kind,
                a,
                b,
                offset,
                angle,
                flip,
            } => {
                let asm = self.assembly_mut(tab)?;
                let known: Vec<InstanceId> = asm.instances.iter().map(|i| i.id).collect();
                let m = asm
                    .mates
                    .iter_mut()
                    .find(|m| m.id == id)
                    .ok_or_else(|| ModelError::Invalid(format!("no mate {}", id.0)))?;
                for c in [&a, &b].into_iter().flatten() {
                    if !known.contains(&c.instance) {
                        return Err(ModelError::Invalid(format!("no instance {}", c.instance.0)));
                    }
                }
                if let Some(n) = name {
                    m.name = n;
                }
                if let Some(k) = kind {
                    m.kind = k;
                }
                if let Some(c) = a {
                    m.a = c;
                }
                if let Some(c) = b {
                    m.b = c;
                }
                if let Some(o) = offset {
                    if !o.is_finite() {
                        return Err(ModelError::Invalid("offset must be finite".into()));
                    }
                    m.offset = o;
                }
                if let Some(v) = angle {
                    if !v.is_finite() {
                        return Err(ModelError::Invalid("angle must be finite".into()));
                    }
                    m.angle = v;
                }
                if let Some(f) = flip {
                    m.flip = f;
                }
                if m.a.instance == m.b.instance {
                    return Err(ModelError::Invalid(
                        "a mate joins two different instances".into(),
                    ));
                }
            }
            AssemblyOp::RemoveMate { id } => {
                let asm = self.assembly_mut(tab)?;
                let pos = asm
                    .mates
                    .iter()
                    .position(|m| m.id == id)
                    .ok_or_else(|| ModelError::Invalid(format!("no mate {}", id.0)))?;
                asm.mates.remove(pos);
            }
            AssemblyOp::Restore { instances, mates } => {
                let max = instances
                    .iter()
                    .map(|i| i.id.0)
                    .chain(mates.iter().map(|m| m.id.0))
                    .max()
                    .unwrap_or(0);
                self.next_id = self.next_id.max(max + 1);
                let asm = self.assembly_mut(tab)?;
                asm.instances = instances;
                asm.mates = mates;
            }
        }
        Ok(())
    }

    /// Applies an op and also returns, in `inverse`, the ops that undo it.
    pub fn apply_with_inverse(
        &mut self,
        op: DocOp,
        base: Option<u32>,
    ) -> Result<DocOpResult, ModelError> {
        // Capture what the op may change.
        let before: Option<Before> = match &op {
            DocOp::RenameTab { tab, .. } => {
                self.tab(*tab).map(|t| Before::Name(t.name().to_string()))
            }
            DocOp::DeleteTab { tab } => {
                self.tabs.iter().position(|t| t.id == *tab).map(|index| {
                    Before::Tab(index, serde_json::to_value(&self.tabs[index]).unwrap())
                })
            }
            DocOp::RenameDocument { .. } => Some(Before::Name(self.name.clone())),
            DocOp::ReplaceDocument { .. } => Some(Before::Json(self.to_json())),
            DocOp::Assembly { tab, op } => self.assembly(*tab).ok().map(|a| match op {
                AssemblyOp::SetInstance { id, .. } => a
                    .instance(*id)
                    .map(|i| Before::Instance(i.clone()))
                    .unwrap_or(Before::None),
                AssemblyOp::SetMate { id, .. } => a
                    .mate(*id)
                    .map(|m| Before::Mate(m.clone()))
                    .unwrap_or(Before::None),
                _ => Before::Assembly(a.instances.clone(), a.mates.clone()),
            }),
            _ => None,
        };
        let mut result = self.apply_with_base(op.clone(), base)?;
        result.inverse = match (op, before) {
            (
                DocOp::AddPartStudio { .. } | DocOp::AddAssembly { .. } | DocOp::InsertTab { .. },
                _,
            ) => result
                .tab
                .map(|tab| vec![DocOp::DeleteTab { tab }])
                .unwrap_or_default(),
            (DocOp::RenameTab { tab, .. }, Some(Before::Name(name))) => {
                vec![DocOp::RenameTab { tab, name }]
            }
            (DocOp::DeleteTab { .. }, Some(Before::Tab(index, tab))) => {
                vec![DocOp::InsertTab { index, tab }]
            }
            (DocOp::RenameDocument { .. }, Some(Before::Name(name))) => {
                vec![DocOp::RenameDocument { name }]
            }
            (DocOp::ReplaceDocument { .. }, Some(Before::Json(json))) => {
                vec![DocOp::ReplaceDocument { json }]
            }
            (DocOp::Studio { tab, .. }, _) => result
                .studio
                .as_ref()
                .map(|r| {
                    r.inverse
                        .iter()
                        .map(|op| DocOp::Studio {
                            tab,
                            op: op.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            (DocOp::Assembly { tab, op }, Some(before)) => match (op, before) {
                (
                    AssemblyOp::SetInstance {
                        id,
                        name,
                        fixed,
                        placement,
                    },
                    Before::Instance(old),
                ) => vec![DocOp::Assembly {
                    tab,
                    op: AssemblyOp::SetInstance {
                        id,
                        name: name.map(|_| old.name.clone()),
                        fixed: fixed.map(|_| old.fixed),
                        placement: placement.map(|_| old.placement),
                    },
                }],
                (
                    AssemblyOp::SetMate {
                        id,
                        name,
                        kind,
                        a,
                        b,
                        offset,
                        angle,
                        flip,
                    },
                    Before::Mate(old),
                ) => vec![DocOp::Assembly {
                    tab,
                    op: AssemblyOp::SetMate {
                        id,
                        name: name.map(|_| old.name.clone()),
                        kind: kind.map(|_| old.kind),
                        a: a.map(|_| old.a),
                        b: b.map(|_| old.b),
                        offset: offset.map(|_| old.offset),
                        angle: angle.map(|_| old.angle),
                        flip: flip.map(|_| old.flip),
                    },
                }],
                (_, Before::Assembly(instances, mates)) => vec![DocOp::Assembly {
                    tab,
                    op: AssemblyOp::Restore { instances, mates },
                }],
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        Ok(result)
    }

    pub fn apply_json_with_base(
        &mut self,
        op: &str,
        base: Option<u32>,
    ) -> Result<DocOpResult, ModelError> {
        let op: DocOp = serde_json::from_str(op).map_err(|e| ModelError::Invalid(e.to_string()))?;
        self.apply_with_base(op, base)
    }

    /// Structural hash over every tab, for replica consistency checks.
    pub fn structural_hash(&self) -> u64 {
        use std::hash::Hasher;
        let mut h = std::hash::DefaultHasher::new();
        h.write_u32(self.tabs.len() as u32);
        for t in &self.tabs {
            h.write_u32(t.id.0);
            match &t.kind {
                TabKind::PartStudio(p) => {
                    h.write_u8(0);
                    h.write_u64(p.structural_hash());
                }
                TabKind::Assembly(a) => {
                    h.write_u8(1);
                    h.write_u32(a.instances.len() as u32);
                    for i in &a.instances {
                        h.write_u32(i.id.0);
                        h.write_u32(i.studio.0);
                        h.write_u32(i.body as u32);
                        h.write_u8(i.fixed as u8);
                    }
                    h.write_u32(a.mates.len() as u32);
                    for m in &a.mates {
                        h.write_u32(m.id.0);
                        h.write_u8(m.kind as u8);
                        for c in [&m.a, &m.b] {
                            h.write_u32(c.instance.0);
                            h.write_u32(c.face.feature.0);
                            h.write_u32(c.face.local);
                        }
                        h.write_u8(m.flip as u8);
                    }
                }
            }
        }
        h.finish()
    }

    /// Regenerates a part studio tab (optionally rolled back).
    pub fn regenerate_studio(
        &mut self,
        tab: TabId,
        rollback: Option<usize>,
    ) -> Result<RegenResult, ModelError> {
        let s = self.studio_mut(tab)?;
        Ok(match rollback {
            Some(n) => s.regenerate_to(n),
            None => s.regenerate(),
        })
    }

    /// Regenerates an assembly tab: every referenced studio first (their
    /// caches make repeats cheap) and every referenced sub-assembly
    /// recursively, then the placement. An assembly that would contain
    /// itself leaves those instances without bodies (reported on them).
    pub fn regenerate_assembly(&mut self, tab: TabId) -> Result<AssemblyResult, ModelError> {
        let mut visiting = vec![tab];
        self.regenerate_assembly_inner(tab, &mut visiting, None)
    }

    /// Resolves an assembly tab as if mate `mate` had the given angle and
    /// offset, without changing the document: the placement a mate
    /// animation shows for one frame.
    pub fn preview_assembly(
        &mut self,
        tab: TabId,
        mate: MateId,
        angle: f64,
        offset: f64,
    ) -> Result<AssemblyResult, ModelError> {
        let mut visiting = vec![tab];
        self.regenerate_assembly_inner(tab, &mut visiting, Some((mate, angle, offset)))
    }

    fn regenerate_assembly_inner(
        &mut self,
        tab: TabId,
        visiting: &mut Vec<TabId>,
        tweak: Option<(MateId, f64, f64)>,
    ) -> Result<AssemblyResult, ModelError> {
        let mut asm = self.assembly(tab)?.clone();
        if let Some((id, angle, offset)) = tweak {
            let m = asm
                .mates
                .iter_mut()
                .find(|m| m.id == id)
                .ok_or_else(|| ModelError::Invalid(format!("no mate {}", id.0)))?;
            m.angle = angle;
            m.offset = offset;
        }
        let mut studios: BTreeMap<TabId, RegenResult> = BTreeMap::new();
        let mut subs: BTreeMap<TabId, AssemblyResult> = BTreeMap::new();
        for inst in &asm.instances {
            let source = inst.studio;
            match self.tab(source).map(|t| t.kind_name()) {
                Some("part_studio") => {
                    if let std::collections::btree_map::Entry::Vacant(e) = studios.entry(source) {
                        if let Ok(r) = self.regenerate_studio(source, None) {
                            e.insert(r);
                        }
                    }
                }
                Some("assembly") => {
                    if !subs.contains_key(&source) && !visiting.contains(&source) {
                        visiting.push(source);
                        if let Ok(r) = self.regenerate_assembly_inner(source, visiting, None) {
                            subs.insert(source, r);
                        }
                        visiting.pop();
                    }
                }
                _ => {}
            }
        }
        let mut solids: BTreeMap<InstanceId, Vec<ok_brep::Solid>> = BTreeMap::new();
        for inst in &asm.instances {
            if let Some(b) = studios
                .get(&inst.studio)
                .and_then(|r| r.bodies.get(inst.body))
            {
                solids.insert(inst.id, vec![b.solid.clone()]);
            } else if let Some(r) = subs.get(&inst.studio) {
                if !r.bodies.is_empty() {
                    solids.insert(inst.id, r.bodies.iter().map(|b| b.solid.clone()).collect());
                }
            }
        }
        Ok(asm.resolve(&solids))
    }
}

enum Before {
    None,
    Name(String),
    Tab(usize, serde_json::Value),
    Json(String),
    Instance(Instance),
    Mate(Mate),
    Assembly(Vec<Instance>, Vec<Mate>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ExtrudeDirection, ExtrudeEnd, FaceRef, FeatureId, PlaneRef, ProfileSelection, SketchOp,
        StandardPlane,
    };
    use ok_math::Vec2;

    fn normalised(json: &str) -> serde_json::Value {
        fn strip(v: &mut serde_json::Value) {
            match v {
                serde_json::Value::Object(m) => {
                    m.remove("next_id");
                    m.remove("next_entity");
                    m.remove("next_constraint");
                    m.values_mut().for_each(strip);
                }
                serde_json::Value::Array(a) => a.iter_mut().for_each(strip),
                _ => {}
            }
        }
        let mut v: serde_json::Value = serde_json::from_str(json).unwrap();
        strip(&mut v);
        v
    }

    fn round_trip(d: &mut Document, op: DocOp) -> DocOpResult {
        let before = d.to_json();
        let r = d.apply_with_inverse(op.clone(), None).unwrap();
        assert!(!r.inverse.is_empty(), "no inverse for {op:?}");
        let mut undone = d.clone();
        for inv in &r.inverse {
            undone.apply_with_base(inv.clone(), None).unwrap();
        }
        assert_eq!(
            normalised(&undone.to_json()),
            normalised(&before),
            "undo of {op:?}"
        );
        r
    }

    /// A document with a studio holding a 10x10x5 block, and an empty assembly.
    fn block_doc() -> (Document, TabId, TabId, FeatureId) {
        let mut d = Document::new("t");
        let studio = d.first_studio().unwrap();
        let s = d
            .apply_with_base(
                DocOp::Studio {
                    tab: studio,
                    op: Op::AddSketch {
                        plane: PlaneRef::standard(StandardPlane::Top),
                        name: None,
                    },
                },
                None,
            )
            .unwrap()
            .studio
            .unwrap()
            .feature
            .unwrap();
        d.apply_with_base(
            DocOp::Studio {
                tab: studio,
                op: Op::Sketch {
                    id: s,
                    op: SketchOp::AddRectangle {
                        a: Vec2::ZERO,
                        b: Vec2::new(10.0, 10.0),
                    },
                },
            },
            None,
        )
        .unwrap();
        let e = d
            .apply_with_base(
                DocOp::Studio {
                    tab: studio,
                    op: Op::AddExtrude {
                        sketch: s,
                        depth: 5.0,
                        direction: ExtrudeDirection::Normal,
                        end: ExtrudeEnd::Blind,
                        profiles: ProfileSelection::All,
                        op: crate::BodyOp::New,
                        name: None,
                    },
                },
                None,
            )
            .unwrap()
            .studio
            .unwrap()
            .feature
            .unwrap();
        let asm = d
            .apply_with_base(DocOp::AddAssembly { name: None }, None)
            .unwrap()
            .tab
            .unwrap();
        (d, studio, asm, e)
    }

    #[test]
    fn legacy_part_studio_json_loads_as_a_document() {
        let json = PartStudio::demo().to_json();
        let d = Document::from_json(&json).unwrap();
        assert_eq!(d.tabs.len(), 1);
        assert_eq!(d.name, "Demo plate");
        assert_eq!(
            d.tab(d.first_studio().unwrap()).unwrap().kind_name(),
            "part_studio"
        );
        let again = Document::from_json(&d.to_json()).unwrap();
        assert_eq!(again.tabs.len(), 1);
    }

    #[test]
    fn assembly_of_two_blocks_regenerates_and_edits_round_trip() {
        let (mut d, studio, asm, e) = block_doc();
        let a = round_trip(
            &mut d,
            DocOp::Assembly {
                tab: asm,
                op: AssemblyOp::AddInstance {
                    studio,
                    body: 0,
                    name: None,
                    fixed: true,
                    placement: Placement::default(),
                },
            },
        )
        .instance
        .unwrap();
        let b = round_trip(
            &mut d,
            DocOp::Assembly {
                tab: asm,
                op: AssemblyOp::AddInstance {
                    studio,
                    body: 0,
                    name: None,
                    fixed: false,
                    placement: Placement::default(),
                },
            },
        )
        .instance
        .unwrap();
        assert_ne!(a, b);
        assert_eq!(
            d.assembly(asm).unwrap().instances[1].name,
            "Part Studio 1 <2>"
        );
        let m = round_trip(
            &mut d,
            DocOp::Assembly {
                tab: asm,
                op: AssemblyOp::AddMate {
                    kind: MateKind::Fastened,
                    a: Connector {
                        instance: a,
                        face: FaceRef {
                            feature: e,
                            local: 1,
                        },
                        anchor: crate::Anchor::Face,
                    },
                    b: Connector {
                        instance: b,
                        face: FaceRef {
                            feature: e,
                            local: 0,
                        },
                        anchor: crate::Anchor::Face,
                    },
                    offset: 0.0,
                    angle: 0.0,
                    flip: false,
                    name: None,
                },
            },
        )
        .mate
        .unwrap();
        let r = d.regenerate_assembly(asm).unwrap();
        assert!(r.mate_errors.is_empty() && r.instance_errors.is_empty());
        assert_eq!(r.bodies.len(), 2);
        let (lo, hi) = r.bodies[1].solid.bounds().unwrap();
        assert!((lo.z - 5.0).abs() < 1e-9 && (hi.z - 10.0).abs() < 1e-9);
        round_trip(
            &mut d,
            DocOp::Assembly {
                tab: asm,
                op: AssemblyOp::SetMate {
                    id: m,
                    name: None,
                    kind: Some(MateKind::Slider),
                    a: None,
                    b: None,
                    offset: Some(3.0),
                    angle: None,
                    flip: None,
                },
            },
        );
        let r = d.regenerate_assembly(asm).unwrap();
        let (lo, _) = r.bodies[1].solid.bounds().unwrap();
        assert!((lo.z - 8.0).abs() < 1e-9, "{lo:?}");
        round_trip(
            &mut d,
            DocOp::Assembly {
                tab: asm,
                op: AssemblyOp::SetInstance {
                    id: a,
                    name: Some("base".into()),
                    fixed: None,
                    placement: Some(Placement {
                        position: ok_math::Vec3::new(0.0, 0.0, 100.0),
                        rotation: ok_math::Vec3::ZERO,
                    }),
                },
            },
        );
        let r = d.regenerate_assembly(asm).unwrap();
        let (lo, _) = r.bodies[1].solid.bounds().unwrap();
        assert!(
            (lo.z - 108.0).abs() < 1e-9,
            "the chain follows the fixed instance: {lo:?}"
        );
        // Removing the instance takes its mate along; undo brings both back.
        round_trip(
            &mut d,
            DocOp::Assembly {
                tab: asm,
                op: AssemblyOp::RemoveInstance { id: b },
            },
        );
        assert!(d.assembly(asm).unwrap().mates.is_empty());
        round_trip(
            &mut d,
            DocOp::RenameTab {
                tab: asm,
                name: "Main".into(),
            },
        );
        round_trip(&mut d, DocOp::RenameDocument { name: "doc".into() });
        round_trip(&mut d, DocOp::DeleteTab { tab: asm });
        assert!(
            d.apply_with_base(DocOp::DeleteTab { tab: studio }, None)
                .is_err(),
            "last tab stays"
        );
        round_trip(&mut d, DocOp::AddPartStudio { name: None });
        assert_eq!(d.tabs.last().unwrap().name(), "Part Studio 2");
        round_trip(
            &mut d,
            DocOp::ReplaceDocument {
                json: Document::new("x").to_json(),
            },
        );
    }

    #[test]
    fn sub_assemblies_are_rigid_groups_and_cycles_are_refused() {
        let (mut d, studio, asm, e) = block_doc();
        let add = |d: &mut Document, tab: TabId, source: TabId, fixed: bool| -> InstanceId {
            d.apply_with_base(
                DocOp::Assembly {
                    tab,
                    op: AssemblyOp::AddInstance {
                        studio: source,
                        body: 0,
                        name: None,
                        fixed,
                        placement: Placement::default(),
                    },
                },
                None,
            )
            .unwrap()
            .instance
            .unwrap()
        };
        // Sub-assembly: two blocks stacked.
        let a = add(&mut d, asm, studio, true);
        let b = add(&mut d, asm, studio, false);
        d.apply_with_base(
            DocOp::Assembly {
                tab: asm,
                op: AssemblyOp::AddMate {
                    kind: MateKind::Fastened,
                    a: Connector {
                        instance: a,
                        face: FaceRef {
                            feature: e,
                            local: 1,
                        },
                        anchor: crate::Anchor::Face,
                    },
                    b: Connector {
                        instance: b,
                        face: FaceRef {
                            feature: e,
                            local: 0,
                        },
                        anchor: crate::Anchor::Face,
                    },
                    offset: 0.0,
                    angle: 0.0,
                    flip: false,
                    name: None,
                },
            },
            None,
        )
        .unwrap();
        // Top-level assembly with the stack inserted twice, the second fastened
        // to the first's top face (the top block's top: feature e, local 1 on
        // the second body of the group resolves to the first body with that
        // face, the bottom block, so use an explicit placement instead).
        let top = d
            .apply_with_base(DocOp::AddAssembly { name: None }, None)
            .unwrap()
            .tab
            .unwrap();
        let s1 = add(&mut d, top, asm, true);
        let s2 = add(&mut d, top, asm, false);
        d.apply_with_base(
            DocOp::Assembly {
                tab: top,
                op: AssemblyOp::SetInstance {
                    id: s2,
                    name: None,
                    fixed: None,
                    placement: Some(Placement {
                        position: ok_math::Vec3::new(20.0, 0.0, 0.0),
                        rotation: ok_math::Vec3::ZERO,
                    }),
                },
            },
            None,
        )
        .unwrap();
        let r = d.regenerate_assembly(top).unwrap();
        assert!(r.instance_errors.is_empty(), "{:?}", r.instance_errors);
        assert_eq!(r.bodies.len(), 4);
        assert_eq!(r.placed, vec![s1, s1, s2, s2]);
        let (lo, hi) = r.bodies[3].solid.bounds().unwrap();
        assert!(
            (lo.x - 20.0).abs() < 1e-9 && (hi.z - 10.0).abs() < 1e-9,
            "{lo:?} {hi:?}"
        );
        let (overlaps, _) = r.interferences();
        assert!(overlaps.is_empty());
        // The sub-assembly cannot contain the top-level one (a cycle): the
        // instance is refused nothing at op time (tabs are valid) but gets
        // no bodies and an error at regeneration.
        let cyc = add(&mut d, asm, top, false);
        let r = d.regenerate_assembly(asm).unwrap();
        assert!(r.instance_errors.contains_key(&cyc));
        assert!(d
            .apply_with_base(
                DocOp::Assembly {
                    tab: asm,
                    op: AssemblyOp::AddInstance {
                        studio: asm,
                        body: 0,
                        name: None,
                        fixed: false,
                        placement: Placement::default()
                    }
                },
                None
            )
            .is_err());
    }

    #[test]
    fn deleting_a_body_from_the_studio_is_reported_on_the_instance() {
        let (mut d, studio, asm, e) = block_doc();
        let a = d
            .apply_with_base(
                DocOp::Assembly {
                    tab: asm,
                    op: AssemblyOp::AddInstance {
                        studio,
                        body: 0,
                        name: None,
                        fixed: true,
                        placement: Placement::default(),
                    },
                },
                None,
            )
            .unwrap()
            .instance
            .unwrap();
        d.apply_with_base(
            DocOp::Studio {
                tab: studio,
                op: Op::SetSuppressed {
                    id: e,
                    suppressed: true,
                },
            },
            None,
        )
        .unwrap();
        let r = d.regenerate_assembly(asm).unwrap();
        assert!(r.bodies.is_empty());
        assert!(r.instance_errors.contains_key(&a));
        let h1 = d.structural_hash();
        d.apply_with_base(
            DocOp::RenameTab {
                tab: asm,
                name: "z".into(),
            },
            None,
        )
        .unwrap();
        assert_eq!(h1, d.structural_hash(), "names are not structural");
    }
}
