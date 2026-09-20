//! Three-way merges of documents.
//!
//! A branch and the document it was made from share a common ancestor: the
//! state at the branch point. `Document::merge_from` expresses what changed
//! on the other side since that ancestor as ops that apply to this
//! document, so the same code merges a branch into its origin and pulls the
//! origin's later work into a branch.
//!
//! The unit of change is a feature (by id) in a part studio, an instance or
//! mate (by id) in an assembly, a tab, or a name. A unit changed on both
//! sides in different ways is a conflict: it is reported and left as it is
//! here. Everything else merges. Feature ids are allotted per document, so
//! a feature added on both sides can carry the same id; that is reported as
//! a conflict too rather than guessed at.

use crate::{
    Assembly, AssemblyOp, DocOp, Document, Feature, FeatureId, Instance, Mate, Op, PartStudio, Tab,
    TabId, TabKind,
};
use std::collections::BTreeMap;

/// The ops that bring the other side's changes in, and what could not be.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Merge {
    pub ops: Vec<DocOp>,
    pub conflicts: Vec<String>,
    /// Number of merged changes (features, instances, mates, tabs, names).
    pub changes: usize,
}

impl Merge {
    fn push(&mut self, op: DocOp) {
        self.ops.push(op);
    }
}

fn same<T: serde::Serialize>(a: &T, b: &T) -> bool {
    serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
}

impl Document {
    /// Ops that apply `theirs`' changes since `base` to this document, with
    /// the conflicts that were left out. Nothing is changed here.
    pub fn merge_from(&self, base: &Document, theirs: &Document) -> Merge {
        let mut m = Merge::default();
        // The document's own name.
        if theirs.name != base.name {
            if self.name == base.name {
                m.push(DocOp::RenameDocument {
                    name: theirs.name.clone(),
                });
                m.changes += 1;
            } else if self.name != theirs.name {
                m.conflicts.push(format!(
                    "document renamed on both sides ({:?} here, {:?} there)",
                    self.name, theirs.name
                ));
            }
        }
        let tab_of = |d: &Document, id: TabId| d.tabs.iter().find(|t| t.id == id).cloned();
        // Tabs in theirs: new ones come over, shared ones merge inside.
        for t in &theirs.tabs {
            let in_base = tab_of(base, t.id);
            let in_ours = tab_of(self, t.id);
            match (in_base, in_ours) {
                (None, None) => {
                    m.push(DocOp::InsertTab {
                        index: usize::MAX,
                        tab: serde_json::to_value(t).unwrap_or_default(),
                    });
                    m.changes += 1;
                }
                (None, Some(o)) => {
                    if !same(&o, t) {
                        m.conflicts.push(format!(
                            "tab {:?} was added on both sides with different content",
                            t.name()
                        ));
                    }
                }
                (Some(b), None) => {
                    if !same(&b, t) {
                        m.conflicts.push(format!(
                            "tab {:?} changed there but was deleted here",
                            t.name()
                        ));
                    }
                }
                (Some(b), Some(o)) => merge_tab(&mut m, &b, &o, t),
            }
        }
        // Tabs deleted there go here too, unless they changed here.
        for b in &base.tabs {
            if tab_of(theirs, b.id).is_some() {
                continue;
            }
            if let Some(o) = tab_of(self, b.id) {
                if same(&o, b) {
                    m.push(DocOp::DeleteTab { tab: b.id });
                    m.changes += 1;
                } else {
                    m.conflicts.push(format!(
                        "tab {:?} was deleted there but changed here",
                        b.name()
                    ));
                }
            }
        }
        m
    }

    /// Applies a merge's ops, reporting the ones this document refused.
    pub fn apply_merge(&mut self, merge: &Merge) -> Vec<String> {
        let mut refused = Vec::new();
        for op in &merge.ops {
            if let Err(e) = self.apply_with_base(op.clone(), None) {
                refused.push(e.to_string());
            }
        }
        refused
    }
}

fn merge_tab(m: &mut Merge, base: &Tab, ours: &Tab, theirs: &Tab) {
    if theirs.name() != base.name() {
        if ours.name() == base.name() {
            m.push(DocOp::RenameTab {
                tab: theirs.id,
                name: theirs.name().to_string(),
            });
            m.changes += 1;
        } else if ours.name() != theirs.name() {
            m.conflicts.push(format!(
                "tab renamed on both sides ({:?} here, {:?} there)",
                ours.name(),
                theirs.name()
            ));
        }
    }
    match (&base.kind, &ours.kind, &theirs.kind) {
        (TabKind::PartStudio(b), TabKind::PartStudio(o), TabKind::PartStudio(t)) => {
            merge_studio(m, theirs.id, b, o, t)
        }
        (TabKind::Assembly(b), TabKind::Assembly(o), TabKind::Assembly(t)) => {
            merge_assembly(m, theirs.id, b, o, t)
        }
        _ => m
            .conflicts
            .push(format!("tab {:?} changed kind", theirs.name())),
    }
}

/// Merges one part studio: features by id, then settings, part names and
/// materials.
fn merge_studio(
    m: &mut Merge,
    tab: TabId,
    base: &PartStudio,
    ours: &PartStudio,
    theirs: &PartStudio,
) {
    let by_id = |s: &PartStudio| -> BTreeMap<FeatureId, Feature> {
        s.features().iter().map(|f| (f.id, f.clone())).collect()
    };
    let (bf, of, tf) = (by_id(base), by_id(ours), by_id(theirs));
    let studio = |op: Op| DocOp::Studio { tab, op };
    // The feature order here as the ops so far leave it, for insert indices.
    let mut order: Vec<FeatureId> = ours.features().iter().map(|f| f.id).collect();
    let mut prev: Option<FeatureId> = None;
    for t in theirs.features() {
        let index_after_prev = |order: &[FeatureId]| {
            prev.and_then(|p| order.iter().position(|&id| id == p))
                .map(|i| i + 1)
                .unwrap_or(0)
        };
        match (bf.get(&t.id), of.get(&t.id)) {
            (None, None) => {
                let index = index_after_prev(&order);
                m.push(studio(Op::InsertFeature {
                    index,
                    feature: t.clone(),
                }));
                order.insert(index, t.id);
                m.changes += 1;
            }
            (None, Some(o)) => {
                if o != t {
                    m.conflicts.push(format!(
                        "{}: feature {:?} was added on both sides with different content",
                        theirs.name, t.name
                    ));
                }
            }
            (Some(b), None) => {
                if b != t {
                    m.conflicts.push(format!(
                        "{}: feature {:?} changed there but was deleted here",
                        theirs.name, t.name
                    ));
                }
            }
            (Some(b), Some(o)) => {
                if b != t {
                    if o == b {
                        let index = order.iter().position(|&id| id == t.id).unwrap_or(0);
                        m.push(studio(Op::DeleteFeature { id: t.id }));
                        m.push(studio(Op::InsertFeature {
                            index,
                            feature: t.clone(),
                        }));
                        m.changes += 1;
                    } else if o != t {
                        m.conflicts.push(format!(
                            "{}: feature {:?} changed on both sides",
                            theirs.name, o.name
                        ));
                    }
                }
            }
        }
        if order.contains(&t.id) {
            prev = Some(t.id);
        }
    }
    for b in base.features() {
        if tf.contains_key(&b.id) {
            continue;
        }
        if let Some(o) = of.get(&b.id) {
            if o == b {
                m.push(studio(Op::DeleteFeature { id: b.id }));
                order.retain(|&id| id != b.id);
                m.changes += 1;
            } else {
                m.conflicts.push(format!(
                    "{}: feature {:?} was deleted there but changed here",
                    theirs.name, o.name
                ));
            }
        }
    }
    if theirs.settings != base.settings {
        if ours.settings == base.settings {
            m.push(studio(Op::SetSettings {
                facet_angle: theirs.settings.facet_angle,
            }));
            m.changes += 1;
        } else if ours.settings != theirs.settings {
            m.conflicts
                .push(format!("{}: settings changed on both sides", theirs.name));
        }
    }
    let ids: std::collections::BTreeSet<FeatureId> = base
        .part_names
        .keys()
        .chain(ours.part_names.keys())
        .chain(theirs.part_names.keys())
        .chain(base.part_materials.keys())
        .chain(ours.part_materials.keys())
        .chain(theirs.part_materials.keys())
        .copied()
        .collect();
    for id in ids {
        let (b, o, t) = (
            base.part_names.get(&id),
            ours.part_names.get(&id),
            theirs.part_names.get(&id),
        );
        if t != b {
            if o == b {
                m.push(studio(Op::RenamePart {
                    source: id,
                    name: t.cloned(),
                }));
                m.changes += 1;
            } else if o != t {
                m.conflicts.push(format!(
                    "{}: a part was renamed on both sides ({:?} here, {:?} there)",
                    theirs.name,
                    o.map(String::as_str).unwrap_or(""),
                    t.map(String::as_str).unwrap_or("")
                ));
            }
        }
        let (b, o, t) = (
            base.part_materials.get(&id),
            ours.part_materials.get(&id),
            theirs.part_materials.get(&id),
        );
        if t != b {
            if o == b {
                m.push(studio(Op::SetPartMaterial {
                    source: id,
                    material: t.cloned(),
                }));
                m.changes += 1;
            } else if o != t {
                m.conflicts.push(format!(
                    "{}: a part's material was changed on both sides",
                    theirs.name
                ));
            }
        }
    }
}

/// Merges one assembly: instances and mates by id, restored as one list
/// when anything differs.
fn merge_assembly(m: &mut Merge, tab: TabId, base: &Assembly, ours: &Assembly, theirs: &Assembly) {
    let mut changes = 0;
    let instances = merge_items(
        &base.instances,
        &ours.instances,
        &theirs.instances,
        |i: &Instance| i.id.0,
        |i| format!("{}: instance {:?}", theirs.name, i.name),
        &mut changes,
        &mut m.conflicts,
    );
    let mates = merge_items(
        &base.mates,
        &ours.mates,
        &theirs.mates,
        |x: &Mate| x.id.0,
        |x| format!("{}: mate {:?}", theirs.name, x.name),
        &mut changes,
        &mut m.conflicts,
    );
    if changes > 0 {
        m.push(DocOp::Assembly {
            tab,
            op: AssemblyOp::Restore { instances, mates },
        });
        m.changes += changes;
    }
}

/// Three-way merge of id-keyed lists; returns the merged list in `theirs`'
/// order for shared items, with ours-only items kept where they were.
fn merge_items<T: Clone + PartialEq>(
    base: &[T],
    ours: &[T],
    theirs: &[T],
    id: impl Fn(&T) -> u32,
    describe: impl Fn(&T) -> String,
    changes: &mut usize,
    conflicts: &mut Vec<String>,
) -> Vec<T> {
    let find = |list: &[T], k: u32| list.iter().find(|x| id(x) == k).cloned();
    let mut out: Vec<T> = ours.to_vec();
    let mut prev: Option<u32> = None;
    for t in theirs {
        let k = id(t);
        let index_after_prev = |out: &[T]| {
            prev.and_then(|p| out.iter().position(|x| id(x) == p))
                .map(|i| i + 1)
                .unwrap_or(0)
        };
        match (find(base, k), find(ours, k)) {
            (None, None) => {
                let index = index_after_prev(&out);
                out.insert(index, t.clone());
                *changes += 1;
            }
            (None, Some(o)) => {
                if o != *t {
                    conflicts.push(format!(
                        "{} was added on both sides with different content",
                        describe(t)
                    ));
                }
            }
            (Some(b), None) => {
                if b != *t {
                    conflicts.push(format!(
                        "{} changed there but was removed here",
                        describe(t)
                    ));
                }
            }
            (Some(b), Some(o)) => {
                if b != *t {
                    if o == b {
                        if let Some(p) = out.iter().position(|x| id(x) == k) {
                            out[p] = t.clone();
                        }
                        *changes += 1;
                    } else if o != *t {
                        conflicts.push(format!("{} changed on both sides", describe(t)));
                    }
                }
            }
        }
        if out.iter().any(|x| id(x) == k) {
            prev = Some(k);
        }
    }
    for b in base {
        let k = id(b);
        if theirs.iter().any(|t| id(t) == k) {
            continue;
        }
        if let Some(o) = find(ours, k) {
            if o == *b {
                out.retain(|x| id(x) != k);
                *changes += 1;
            } else {
                conflicts.push(format!(
                    "{} was removed there but changed here",
                    describe(b)
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Op, Placement};

    fn add_var(doc: &mut Document, tab: TabId, name: &str, expr: &str) -> FeatureId {
        let r = doc
            .apply_with_base(
                DocOp::Studio {
                    tab,
                    op: Op::AddVariable {
                        name: name.to_string(),
                        expression: expr.to_string(),
                    },
                },
                None,
            )
            .unwrap();
        r.studio.unwrap().feature.unwrap()
    }

    fn names(doc: &Document, tab: TabId) -> Vec<String> {
        doc.studio(tab)
            .unwrap()
            .features()
            .iter()
            .map(|f| f.name.clone())
            .collect()
    }

    #[test]
    fn merges_independent_feature_edits_and_reports_conflicts() {
        let mut base = Document::new("doc");
        let tab = base.tabs[0].id;
        let a = add_var(&mut base, tab, "a", "1");
        let b = add_var(&mut base, tab, "b", "2");
        let c = add_var(&mut base, tab, "c", "3");
        // Ours: change a, delete c, add x. Theirs: change b, add y after b,
        // change c (conflict with our delete), rename the document.
        let mut ours = base.clone();
        ours.apply_with_base(
            DocOp::Studio {
                tab,
                op: Op::SetVariable {
                    id: a,
                    name: None,
                    expression: Some("10".into()),
                },
            },
            None,
        )
        .unwrap();
        ours.apply_with_base(
            DocOp::Studio {
                tab,
                op: Op::DeleteFeature { id: c },
            },
            None,
        )
        .unwrap();
        add_var(&mut ours, tab, "x", "0");
        let mut theirs = base.clone();
        theirs
            .apply_with_base(
                DocOp::Studio {
                    tab,
                    op: Op::SetVariable {
                        id: b,
                        name: None,
                        expression: Some("20".into()),
                    },
                },
                None,
            )
            .unwrap();
        theirs
            .apply_with_base(
                DocOp::Studio {
                    tab,
                    op: Op::SetVariable {
                        id: c,
                        name: None,
                        expression: Some("30".into()),
                    },
                },
                None,
            )
            .unwrap();
        // Their new feature gets an id that collides with our "x": both
        // documents allot ids from the same counter. Give it a distinct one
        // by inserting explicitly, as a branch client's prefix would.
        let mut y = theirs.studio(tab).unwrap().features()[0].clone();
        y.id = FeatureId(77);
        y.name = "#y".into();
        theirs
            .apply_with_base(
                DocOp::Studio {
                    tab,
                    op: Op::InsertFeature {
                        index: 2,
                        feature: y,
                    },
                },
                None,
            )
            .unwrap();
        theirs
            .apply_with_base(
                DocOp::RenameDocument {
                    name: "renamed".into(),
                },
                None,
            )
            .unwrap();

        let merge = ours.merge_from(&base, &theirs);
        assert_eq!(merge.changes, 3, "{merge:?}");
        assert_eq!(merge.conflicts.len(), 1, "{merge:?}");
        assert!(merge.conflicts[0].contains("deleted here"));
        let refused = ours.apply_merge(&merge);
        assert!(refused.is_empty(), "{refused:?}");
        assert_eq!(ours.name, "renamed");
        assert_eq!(names(&ours, tab), ["#a", "#b", "#y", "#x"]);
        let expr = |id: FeatureId| match &ours.studio(tab).unwrap().feature(id).unwrap().kind {
            crate::FeatureKind::Variable(v) => v.expression.clone(),
            _ => unreachable!(),
        };
        assert_eq!(expr(a), "10");
        assert_eq!(expr(b), "20");
        // Merging the same branch again changes nothing more.
        let again = ours.merge_from(&base, &theirs);
        assert_eq!(again.changes, 0, "{again:?}");
    }

    #[test]
    fn merges_tabs_names_and_assemblies() {
        let mut base = Document::new("doc");
        let studio = base.tabs[0].id;
        add_var(&mut base, studio, "a", "1");
        let asm = base
            .apply_with_base(DocOp::AddAssembly { name: None }, None)
            .unwrap()
            .tab
            .unwrap();
        let inst = base
            .apply_with_base(
                DocOp::Assembly {
                    tab: asm,
                    op: AssemblyOp::AddInstance {
                        studio,
                        body: 0,
                        name: Some("one".into()),
                        fixed: true,
                        placement: Placement::default(),
                    },
                },
                None,
            )
            .unwrap()
            .instance
            .unwrap();
        let mut ours = base.clone();
        let mut theirs = base.clone();
        // Theirs renames the studio, adds a tab, moves the instance.
        theirs
            .apply_with_base(
                DocOp::RenameTab {
                    tab: studio,
                    name: "Main".into(),
                },
                None,
            )
            .unwrap();
        theirs
            .apply_with_base(DocOp::AddPartStudio { name: None }, None)
            .unwrap();
        theirs
            .apply_with_base(
                DocOp::Assembly {
                    tab: asm,
                    op: AssemblyOp::SetInstance {
                        id: inst,
                        name: Some("moved".into()),
                        fixed: None,
                        placement: None,
                    },
                },
                None,
            )
            .unwrap();
        // Ours also adds a tab (same id as theirs: a conflict) and renames
        // the instance differently (conflict).
        ours.apply_with_base(DocOp::AddAssembly { name: None }, None)
            .unwrap();
        ours.apply_with_base(
            DocOp::Assembly {
                tab: asm,
                op: AssemblyOp::SetInstance {
                    id: inst,
                    name: Some("mine".into()),
                    fixed: None,
                    placement: None,
                },
            },
            None,
        )
        .unwrap();
        let merge = ours.merge_from(&base, &theirs);
        assert_eq!(merge.changes, 1, "{merge:?}");
        assert_eq!(merge.conflicts.len(), 2, "{merge:?}");
        assert!(ours.apply_merge(&merge).is_empty());
        assert_eq!(ours.tab(studio).unwrap().name(), "Main");
        assert_eq!(ours.tabs.len(), 3);
        assert_eq!(ours.assembly(asm).unwrap().instances[0].name, "mine");
    }
}
