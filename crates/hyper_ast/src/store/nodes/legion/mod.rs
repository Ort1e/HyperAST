use std::fmt::Debug;
use std::marker::{Send, Sync};
use std::ops::Deref;

use legion::storage::{Component, IntoComponentSource};
use legion::{EntityStore, World};

use super::{Compo, CompoRegister, ErasedHolder, ErasedInserter};

pub mod dyn_builder;
mod elem;
pub use elem::{EntryRef, HashedNodeRef, NodeIdentifier};

mod node_store_impl;

mod stores_impl;

pub type DedupInner = crate::compat::HashMap<NodeIdentifier, (), ()>;

pub struct NodeStore<I = NodeStoreInner, D = DedupInner> {
    #[doc(hidden)]
    pub dedup: D,
    #[doc(hidden)]
    pub inner: I,
}

#[derive(Default)]
pub struct NodeStoreInner {
    count: usize,
    errors: usize,
    #[cfg(feature = "subtree-stats")]
    stats: super::NodeStoreStats,
    // roots: HashMap<(u8, u8, u8), NodeIdentifier>,
    // dedup: Dedup,
    internal: legion::World,
    // TODO intern lists of [`NodeIdentifier`]s, e.g. children, no space children, ...
    // hasher: hashbrown::hash_map::DefaultHashBuilder,
    //fasthash::city::Hash64,//fasthash::RandomState<fasthash::>,

    // internal: VecMapStore<HashedNode, NodeIdentifier, legion::World>,
    hasher: std::hash::BuildHasherDefault<MyNoHashH>,
}

#[derive(Default)]
struct MyNoHashH(u32);

impl std::hash::Hasher for MyNoHashH {
    #[inline]
    fn finish(&self) -> u64 {
        ((self.0 as u64) << 32) | (self.0 as u64)
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.0 = i
    }

    fn write(&mut self, _bytes: &[u8]) {
        unimplemented!()
    }
}

pub struct PendingInsert<'a>(
    crate::compat::hash_map::RawEntryMut<'a, legion::Entity, (), ()>,
    (u64, &'a mut NodeStoreInner),
);

pub struct TypedNode<T, Ty>(T, Ty);

impl<T, Ty: Copy> TypedNode<T, Ty> {
    pub fn get_type(&self) -> Ty {
        self.1
    }
    pub fn into(self) -> T {
        self.0
    }
}
impl<T, Ty> Deref for TypedNode<T, Ty> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn _resolve<'a, T>(
    slf: &'a legion::World,
    id: &NodeIdentifier,
) -> Result<HashedNodeRef<'a, T>, legion::world::EntityAccessError> {
    slf.entry_ref(*id).map(|x| HashedNodeRef::new(x))
}

#[derive(Default)]
pub struct DedupMap(pub DedupInner);

fn default_dedup_hashmap() -> DedupInner {
    crate::compat::HashMap::with_capacity_and_hasher(1 << 21, ())
}

impl NodeStoreInner {
    pub fn make_dedup_map() -> DedupMap {
        DedupMap(default_dedup_hashmap())
    }
}

impl NodeStore {
    pub fn new() -> Self {
        Self {
            inner: NodeStoreInner::default(),
            dedup: default_dedup_hashmap(),
        }
    }
}

impl crate::types::NStore for NodeStore {
    type IdN = NodeIdentifier;
    type Idx = u16;
}

impl<'a> crate::types::lending::NLending<'a, NodeIdentifier> for NodeStore {
    type N = HashedNodeRef<'a, NodeIdentifier>;
}

impl crate::types::NStoreRefAssoc for NodeStore {
    type S = NodeStoreInner;
}

impl<'a> crate::types::lending::NLending<'a, NodeIdentifier> for NodeStoreInner {
    type N = HashedNodeRef<'a, NodeIdentifier>;
}

impl crate::types::NStore for NodeStoreInner {
    type IdN = NodeIdentifier;
    type Idx = u16;
}

impl<'a> crate::types::lending::NLending<'a, NodeIdentifier> for &NodeStoreInner {
    type N = HashedNodeRef<'a, NodeIdentifier>;
}

impl crate::types::NStore for &NodeStoreInner {
    type IdN = NodeIdentifier;
    type Idx = u16;
}

pub fn eq_node<'a, K, L, I>(
    kind: &'a K,
    label_id: Option<&'a L>,
    children: &'a [I],
) -> impl Fn(EntryRef) -> bool + 'a
where
    K: 'static + Eq + Copy + Send + Sync,
    L: 'static + Eq + Copy + Send + Sync,
    I: 'static + Eq + Copy + Send + Sync,
{
    move |x: EntryRef| {
        let t = x.get_component::<K>();
        if t != Ok(kind) {
            return false;
        }
        let l = x.get_component::<L>().ok();
        if l != label_id {
            return false;
        }
        eq_node_cs(children)(x)
    }
}

pub fn eq_node_cs<I>(children: &[I]) -> impl Fn(EntryRef) -> bool + '_
where
    I: 'static + Eq + Copy + Send + Sync,
{
    move |x: EntryRef| {
        use crate::store::nodes::compo;

        if children.len() == 1 {
            let Ok(cs) = x.get_component::<compo::CS0<I, 1>>() else {
                return false;
            };
            cs.0[0] == children[0]
        } else if children.len() == 2 {
            let Ok(cs) = x.get_component::<compo::CS0<I, 2>>() else {
                return false;
            };
            cs.0[..] == children[..]
        } else if !children.is_empty() {
            let Ok(cs) = x.get_component::<compo::CS<I>>() else {
                return false;
            };
            cs.0.as_ref() == children
        } else {
            true
        }
    }
}

/// Same as [`eq_node`], and the fields (roles) stored in the node are the given ones.
///
/// A child's field is stored in its parent (`compo::Roles` and `compo::RoleOffsets`). Two nodes
/// with the same kind, label and children but different fields are different nodes: e.g. the
/// hidden `_string` wrapper of a `string_literal` is `value:` of a cast in one place and `right:`
/// of an assignment in another. The hash does not depend on the fields, so the nodes only share
/// a bucket of the deduplication table.
pub fn eq_node_roles<'a, K, L, I, R>(
    kind: &'a K,
    label_id: Option<&'a L>,
    children: &'a [I],
    roles: &'a [R],
    offsets: &'a [u8],
) -> impl Fn(EntryRef) -> bool + 'a
where
    K: 'static + Eq + Copy + Send + Sync,
    L: 'static + Eq + Copy + Send + Sync,
    I: 'static + Eq + Copy + Send + Sync,
    R: 'static + PartialEq + Send + Sync,
{
    let eq = eq_node(kind, label_id, children);
    move |x: EntryRef| roles_match(&x, roles, offsets) && eq(x)
}

/// The node stores exactly these fields (none when `roles` is empty, as `RoleAcc::add_md`).
pub fn eq_roles<'a, R>(roles: &'a [R], offsets: &'a [u8]) -> impl Fn(EntryRef) -> bool + 'a
where
    R: 'static + PartialEq + Send + Sync,
{
    move |x: EntryRef| roles_match(&x, roles, offsets)
}

fn roles_match<R>(x: &EntryRef, roles: &[R], offsets: &[u8]) -> bool
where
    R: 'static + PartialEq + Send + Sync,
{
    use crate::store::nodes::compo;
    let stored_roles = x.get_component::<compo::Roles<R>>().ok();
    let stored_offsets = x.get_component::<compo::RoleOffsets>().ok();
    match (stored_roles, stored_offsets) {
        (Some(r), Some(o)) => r.0.as_ref() == roles && o.0.as_ref() == offsets,
        (None, None) => roles.is_empty(),
        _ => false,
    }
}

impl ErasedHolder for legion::world::Entry<'_> {
    fn unerase_ref<T: 'static + Send + Sync>(&self, tid: std::any::TypeId) -> Option<&T> {
        if tid == std::any::TypeId::of::<T>() {
            self.get_component().ok()
        } else {
            None
        }
    }
}

impl ErasedInserter for legion::world::Entry<'_> {
    fn insert<T: 'static + Compo>(&mut self, t: T) {
        self.add_component(t);
    }
}

impl CompoRegister for World {
    type Id = legion::storage::ComponentTypeId;

    fn register_compo<T: 'static + Compo>(&mut self) -> Self::Id {
        legion::storage::ComponentTypeId::of::<T>()
    }
}

pub type RawHAST<'hast, TS> =
    crate::store::SimpleStores<TS, &'hast NodeStoreInner, &'hast crate::store::labels::LabelStore>;

pub fn subtree_builder<TS: crate::types::ETypeStore>(ty: TS::Ty) -> dyn_builder::EntityBuilder
where
    <TS::Ty2 as crate::types::TypeTrait>::Lang: Component,
    TS::Ty: Component,
{
    use crate::types::Lang;
    let l = <TS::Ty2 as crate::types::TypeTrait>::Lang::INST;
    let mut builder = dyn_builder::EntityBuilder::with_lang(l);
    builder.add(ty);
    builder
}
