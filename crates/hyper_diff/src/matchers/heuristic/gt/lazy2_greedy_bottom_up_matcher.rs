//! makes greedy_bottom_up_matcher lazy
//! - [ ] first make post order iterator lazy
//!
use std::{fmt::Debug, marker::PhantomData};

use num_traits::{cast, one, PrimInt};

use crate::decompressed_tree_store::{
    ContiguousDescendants, DecompressedTreeStore, DecompressedWithParent,
    LazyDecompressedTreeStore, LazyPOBorrowSlice, PostOrder, PostOrderIterable, PostOrderKeyRoots,
    Shallow, ShallowDecompressedTreeStore,
};
use crate::matchers::mapping_store::MonoMappingStore;
use crate::matchers::Mapper;
use crate::matchers::{optimal::zs::ZsMatcher, similarity_metrics};
use hyperast::types::{DecompressedSubtree, HyperAST, NodeStore, Tree, WithHashs, WithStats};

use crate::decompressed_tree_store::SimpleZsTree as ZsTree;

/// TODO wait for `#![feature(adt_const_params)]` #95174 to be improved
///
/// it will allow to make use complex types as const generics
/// ie. make the different threshold neater
pub struct GreedyBottomUpMatcher<
    'a,
    Dsrc,
    Ddst,
    HAST: HyperAST<'a>,
    M: MonoMappingStore,
    MZs: MonoMappingStore = M,
    const SIZE_THRESHOLD: usize = 1000,
    const SIM_THRESHOLD_NUM: u64 = 1,
    const SIM_THRESHOLD_DEN: u64 = 2,
> {
    label_store: &'a HAST::LS,
    internal: Mapper<'a, HAST, Dsrc, Ddst, M>,
    _phantom: PhantomData<*const MZs>,
}

/// Enable using a slice instead of recreating a ZsTree for each call to ZsMatch, see last_chance_match
const SLICE: bool = true;

// impl<
//         'a,
//         Dsrc,
//         Ddst,
//         HAST,
//         // T: 'a + Tree + WithHashs,
//         // S,
//         // LS: LabelStore<SlicedLabel, I = T::Label>,
//         M: MonoMappingStore,
//         MZs: MonoMappingStore<Src = M::Src, Dst = M::Dst>,
//         const SIZE_THRESHOLD: usize,
//         const SIM_THRESHOLD_NUM: u64,
//         const SIM_THRESHOLD_DEN: u64,
//     > Into<BottomUpMatcher<'a, Dsrc, Ddst, T, S, M>>
//     for GreedyBottomUpMatcher<
//         'a,
//         Dsrc,
//         Ddst,
//         HAST,
//         M,
//         MZs,
//         SIZE_THRESHOLD,
//         SIM_THRESHOLD_NUM,
//         SIM_THRESHOLD_DEN,
//     >
// {
//     fn into(self) -> BottomUpMatcher<'a, Dsrc, Ddst, T, S, M> {
//         self.internal
//     }
// }

// impl<
//         'a,
//         Dsrc,
//         Ddst,
//         T: 'a + Tree + WithHashs,
//         S,
//         LS: LabelStore<SlicedLabel, I = T::Label>,
//         M: MonoMappingStore,
//         const SIZE_THRESHOLD: usize,  // = 1000,
//         const SIM_THRESHOLD_NUM: u64, // = 1,
//         const SIM_THRESHOLD_DEN: u64, // = 2,
//     >
//     GreedyBottomUpMatcher<
//         'a,
//         Dsrc,
//         Ddst,
//         T,
//         S,
//         LS,
//         M,
//         M,
//         SIZE_THRESHOLD,
//         SIM_THRESHOLD_NUM,
//         SIM_THRESHOLD_DEN,
//     >
// {
//     pub fn new(
//         node_store: &'a S,
//         label_store: &'a LS,
//         src_arena: Dsrc,
//         dst_arena: Ddst,
//         mappings: M,
//     ) -> Self {
//         Self {
//             label_store,
//             internal: BottomUpMatcher {
//                 node_store,
//                 src_arena,
//                 dst_arena,
//                 mappings,
//                 _phantom: PhantomData,
//             },
//             _phantom: PhantomData,
//         }
//     }
// }

/// TODO PostOrder might not be necessary
impl<
        'a,
        Dsrc: DecompressedTreeStore<'a, HAST::T, Dsrc::IdD, M::Src>
            + DecompressedWithParent<'a, HAST::T, Dsrc::IdD>
            + PostOrder<'a, HAST::T, Dsrc::IdD, M::Src>
            + PostOrderIterable<'a, HAST::T, Dsrc::IdD, M::Src>
            + DecompressedSubtree<'a, HAST::T>
            + ContiguousDescendants<'a, HAST::T, Dsrc::IdD, M::Src>
            + LazyPOBorrowSlice<'a, HAST::T, Dsrc::IdD, M::Src>
            + ShallowDecompressedTreeStore<'a, HAST::T, Dsrc::IdD, M::Src>
            + LazyDecompressedTreeStore<'a, HAST::T, M::Src>,
        Ddst: DecompressedTreeStore<'a, HAST::T, Ddst::IdD, M::Dst>
            + DecompressedWithParent<'a, HAST::T, Ddst::IdD>
            + PostOrder<'a, HAST::T, Ddst::IdD, M::Dst>
            + PostOrderIterable<'a, HAST::T, Ddst::IdD, M::Dst>
            + DecompressedSubtree<'a, HAST::T>
            + ContiguousDescendants<'a, HAST::T, Ddst::IdD, M::Dst>
            + LazyPOBorrowSlice<'a, HAST::T, Ddst::IdD, M::Dst>
            + ShallowDecompressedTreeStore<'a, HAST::T, Ddst::IdD, M::Dst>
            + LazyDecompressedTreeStore<'a, HAST::T, M::Dst>,
        HAST: HyperAST<'a>,
        M: MonoMappingStore,
        MZs: MonoMappingStore<Src = Dsrc::IdD, Dst = Ddst::IdD> + Default,
        const SIZE_THRESHOLD: usize,
        const SIM_THRESHOLD_NUM: u64,
        const SIM_THRESHOLD_DEN: u64,
    >
    GreedyBottomUpMatcher<
        'a,
        Dsrc,
        Ddst,
        HAST,
        M,
        MZs,
        SIZE_THRESHOLD,
        SIM_THRESHOLD_NUM,
        SIM_THRESHOLD_DEN,
    >
where
    HAST::T: 'a + Tree + WithHashs + WithStats,
    HAST::IdN: 'a + Clone + Eq + Debug,
    // <HAST::T as Typed>::Type: Copy + Eq + Send + Sync,
    Dsrc::IdD: 'a + PrimInt + std::ops::SubAssign + Debug,
    Ddst::IdD: 'a + PrimInt + std::ops::SubAssign + Debug,
    M::Src: 'a + PrimInt + std::ops::SubAssign + Debug,
    M::Dst: 'a + PrimInt + std::ops::SubAssign + Debug,
{
    pub fn match_it(
        mapping: crate::matchers::Mapper<'a, HAST, Dsrc, Ddst, M>,
    ) -> crate::matchers::Mapper<'a, HAST, Dsrc, Ddst, M> {
        let mut matcher = Self {
            label_store: mapping.hyperast.label_store(),
            internal: mapping,
            _phantom: PhantomData,
        };
        matcher.internal.mapping.mappings.topit(
            matcher.internal.mapping.src_arena.len(),
            matcher.internal.mapping.dst_arena.len(),
        );
        Self::execute(&mut matcher.internal, &matcher.label_store);
        matcher.internal
    }

    pub fn execute<'b>(internal: &mut Mapper<'a, HAST, Dsrc, Ddst, M>, _label_store: &'a HAST::LS) {
        assert_eq!(
            // TODO move it inside the arena ...
            internal.src_arena.root(),
            cast::<_, M::Src>(internal.src_arena.len()).unwrap() - one()
        );
        assert!(internal.src_arena.len() > 0);
        // println!("mappings={}", internal.mappings.len());
        // // WARN it is in postorder and it depends on decomp store
        // // -1 as root is handled after forloop
        for a in internal.src_arena.iter_df_post::<false>() {
            // if internal.src_arena.parent(&a).is_none() {
            //     break;
            // }
            if internal.mappings.is_src(&a) {
                continue;
            }
            let a = internal
                .mapping
                .src_arena
                .decompress_to(internal.hyperast.node_store(), &a);
            if Self::src_has_children(internal, a) {
                let candidates = internal.get_dst_candidates_lazily(&a);
                let mut best = None;
                let mut max: f64 = -1.;
                for cand in candidates {
                    let sim = similarity_metrics::SimilarityMeasure::range(
                        &internal.src_arena.descendants_range(&a),
                        &internal.dst_arena.descendants_range(&cand),
                        &internal.mappings,
                    )
                    .dice();
                    if sim > max && sim >= SIM_THRESHOLD_NUM as f64 / SIM_THRESHOLD_DEN as f64 {
                        max = sim;
                        best = Some(cand);
                    }
                }

                if let Some(best) = best {
                    Self::last_chance_match_zs(internal, a, best);
                    internal.mappings.link(*a.shallow(), *best.shallow());
                }
            }
        }
        // for root
        internal.mapping.mappings.link(
            internal.mapping.src_arena.root(),
            internal.mapping.dst_arena.root(),
        );
        let src = internal.src_arena.starter();
        let dst = internal.dst_arena.starter();
        Self::last_chance_match_zs(internal, src, dst);
        // println!("nodes:{}", c);
        // println!("nodes:{}", c2);
    }

    fn src_has_children(internal: &Mapper<'a, HAST, Dsrc, Ddst, M>, src: Dsrc::IdD) -> bool {
        let o = internal.src_arena.original(&src);
        let r = internal.hyperast.node_store().resolve(&o).has_children();

        // TODO put it back
        // debug_assert_eq!(
        //     r,
        //     internal.src_arena.lld(&src) < *src.shallow(),
        //     "{:?} {:?}",
        //     internal.src_arena.lld(&src),
        //     src.to_usize()
        // );
        r
    }

    pub(crate) fn last_chance_match_zs(
        internal: &mut Mapper<'a, HAST, Dsrc, Ddst, M>,
        src: Dsrc::IdD,
        dst: Ddst::IdD,
    ) {
        // allow using another internal mapping store
        // WIP https://blog.rust-lang.org/2022/10/28/gats-stabilization.html#implied-static-requirement-from-higher-ranked-trait-bounds
        let node_store = internal.hyperast.node_store();
        let mapping = &mut internal.mapping;
        let src_arena = &mut mapping.src_arena;
        let dst_arena = &mut mapping.dst_arena;
        let src_s = src_arena.descendants_count(node_store, &src);
        let dst_s = dst_arena.descendants_count(node_store, &dst);
        if !(src_s < cast(SIZE_THRESHOLD).unwrap() || dst_s < cast(SIZE_THRESHOLD).unwrap()) {
            // dbg!(src_s, dst_s);
            return;
        }
        // let label_store = self.label_store;
        let src_offset;
        let dst_offset;
        let zs_mappings: MZs = if SLICE {
            let src_arena = src_arena.slice_po(node_store, &src);
            src_offset = src - src_arena.root();
            let dst_arena = dst_arena.slice_po(node_store, &dst);
            dst_offset = dst - dst_arena.root();
            ZsMatcher::match_with(internal.hyperast, src_arena, dst_arena)
        } else {
            let o_src = src_arena.original(&src);
            let o_dst = dst_arena.original(&dst);
            let src_arena = ZsTree::<HAST::T, Dsrc::IdD>::decompress(node_store, &o_src);
            src_offset = src - src_arena.root();
            if cfg!(debug_assertions) {
                let src_arena_z = mapping.src_arena.slice_po(node_store, &src);
                for i in src_arena.iter_df_post::<true>() {
                    assert_eq!(src_arena.tree(&i), src_arena_z.tree(&i));
                    assert_eq!(src_arena.lld(&i), src_arena_z.lld(&i));
                }
                let mut last = src_arena_z.root();
                for k in src_arena_z.iter_kr() {
                    assert!(src_arena.kr[k.to_usize().unwrap()]);
                    last = k;
                }
                assert!(src_arena.kr[src_arena.kr.len() - 1]);
                dbg!(last == src_arena_z.root());
            }
            let dst_arena = ZsTree::<HAST::T, Ddst::IdD>::decompress(node_store, &o_dst);
            dst_offset = dst - dst_arena.root();
            if cfg!(debug_assertions) {
                let dst_arena_z = mapping.dst_arena.slice_po(node_store, &dst);
                for i in dst_arena.iter_df_post::<true>() {
                    assert_eq!(dst_arena.tree(&i), dst_arena_z.tree(&i));
                    assert_eq!(dst_arena.lld(&i), dst_arena_z.lld(&i));
                }
                let mut last = dst_arena_z.root();
                for k in dst_arena_z.iter_kr() {
                    assert!(dst_arena.kr[k.to_usize().unwrap()]);
                    last = k;
                }
                assert!(dst_arena.kr[dst_arena.kr.len() - 1]);
                dbg!(last == dst_arena_z.root());
            }
            ZsMatcher::match_with(internal.hyperast, src_arena, dst_arena)
        };
        use num_traits::ToPrimitive;
        assert_eq!(
            mapping.src_arena.first_descendant(&src).to_usize(),
            src_offset.to_usize()
        );
        let mappings = &mut mapping.mappings;
        for (i, t) in zs_mappings.iter() {
            //remapping
            let src: Dsrc::IdD = src_offset + cast(i).unwrap();
            let dst: Ddst::IdD = dst_offset + cast(t).unwrap();
            // use it
            if !mappings.is_src(src.shallow()) && !mappings.is_dst(dst.shallow()) {
                let tsrc = internal
                    .hyperast
                    .resolve_type(&mapping.src_arena.original(&src));
                let tdst = internal
                    .hyperast
                    // .resolve(&matcher.src_arena.tree(&t))
                    .resolve_type(&mapping.dst_arena.original(&dst));
                if tsrc == tdst {
                    mappings.link(*src.shallow(), *dst.shallow());
                }
            }
        }
    }
}
