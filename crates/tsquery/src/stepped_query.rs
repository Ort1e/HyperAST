//! Attempt to integrate another query matchers compatible with hyperast.
//! The query matcher used here is largely inspired by tree_sitter (query.c).

use hyperast::types::{
    HyperAST, HyperASTShared, RoleStore, WithPrecompQueries, WithRoles, WithSerialization,
    WithStats,
};
use std::fmt::Debug;

use crate::{ArrayStr, CaptureId};

#[repr(transparent)]
pub struct Node<'tree, HAST: HyperASTShared>(crate::hyperast_cursor::Node<'tree, HAST>);

impl<'tree, HAST: HyperAST<'tree>> Clone for Node<'tree, HAST> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<'tree, HAST: HyperAST<'tree>> PartialEq for Node<'tree, HAST> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<'tree, HAST: HyperAST<'tree>> crate::Node for Node<'tree, HAST>
where
    HAST::IdN: std::fmt::Debug + Copy,
    HAST::TS: RoleStore,
    HAST::T: WithRoles,
    HAST::T: WithPrecompQueries,
{
    fn symbol(&self) -> crate::Symbol {
        self.0.symbol()
    }

    fn is_named(&self) -> bool {
        self.0.is_named()
    }

    fn str_symbol(&self) -> &str {
        self.0.str_symbol()
    }

    fn start_point(&self) -> tree_sitter::Point {
        self.0.start_point()
    }

    type IdF = <HAST::TS as RoleStore>::IdF;

    // fn child_by_field_id(&self, field_id: Self::IdF) -> Option<Self> {
    //     // self.0.child_by_field_id(field_id).map(|x| Self(x))
    // }

    fn has_child_with_field_id(&self, field_id: Self::IdF) -> bool {
        self.0.has_child_with_field_id(field_id)
    }

    fn equal(&self, other: &Self) -> bool {
        self.0.equal(&other.0)
    }

    fn compare(&self, other: &Self) -> std::cmp::Ordering {
        self.0.compare(&other.0)
    }

    type TP<'a> = ();
    fn text(&self, text_provider: Self::TP<'_>) -> std::borrow::Cow<str> {
        self.0.text(text_provider)
    }
}

impl<'tree, HAST: HyperAST<'tree>> crate::Cursor for Node<'tree, HAST>
where
    HAST::IdN: std::fmt::Debug + Copy,
    HAST::TS: RoleStore,
    HAST::T: WithRoles,
    HAST::T: WithPrecompQueries,
{
    type Node = Self;
    type NodeRef<'a>
        = &'a Self
    where
        Self: 'a;

    fn goto_next_sibling_internal(&mut self) -> crate::TreeCursorStep {
        self.0.goto_next_sibling_internal()
    }

    fn goto_first_child_internal(&mut self) -> crate::TreeCursorStep {
        self.0.goto_first_child_internal()
    }

    fn goto_parent(&mut self) -> bool {
        self.0.goto_parent()
    }

    fn current_node(&self) -> &Self {
        self
    }

    fn parent_is_error(&self) -> bool {
        self.0.parent_is_error()
    }

    fn has_parent(&self) -> bool {
        let mut node = self.clone();
        node.goto_parent()
    }

    fn persist(&mut self) -> Self::Node {
        self.clone()
    }

    fn persist_parent(&mut self) -> Option<Self::Node> {
        let mut node = self.clone();
        node.goto_parent();
        Some(node)
    }

    type Status = crate::hyperast_cursor::CursorStatus<<Self as crate::Node>::IdF>;

    fn current_status(&self) -> Self::Status {
        self.0.current_status()
    }

    fn text_provider(&self) -> <Self::Node as crate::Node>::TP<'_> {
        self.0.text_provider()
    }

    fn is_visible_at_root(&self) -> bool {
        self.0.is_visible_at_root()
    }
}

impl<'tree, HAST: HyperAST<'tree>> Node<'tree, HAST> {
    pub fn new(
        stores: &'tree HAST,
        pos: hyperast::position::StructuralPosition<HAST::IdN, HAST::Idx>,
    ) -> Self {
        Self(crate::hyperast_cursor::Node { stores, pos })
    }
}

pub struct MyNodeErazing<'hast, HAST>(std::marker::PhantomData<&'hast HAST>);
impl<'hast, HAST> Default for MyNodeErazing<'hast, HAST> {
    fn default() -> Self {
        Self(Default::default())
    }
}

#[cfg(feature = "tsg")]
impl<'hast, HAST: 'static + hyperast::types::HyperAST<'hast>> tree_sitter_graph::graph::Erzd
    for MyNodeErazing<'hast, HAST>
{
    type Original<'tree> = Node<'tree, HAST>;
}

#[cfg(feature = "tsg")]
impl<'tree, HAST: 'static + HyperAST<'tree>> tree_sitter_graph::graph::LErazng
    for Node<'tree, HAST>
{
    type LErazing = MyNodeErazing<'tree, HAST>;
}

pub struct QueryMatcher<HAST> {
    pub query: crate::Query,
    _phantom: std::marker::PhantomData<HAST>,
}

impl<HAST> QueryMatcher<HAST> {
    fn new(
        source: &str,
        language: &tree_sitter::Language,
    ) -> Result<Self, tree_sitter::QueryError> {
        let query = crate::Query::new(source, language.clone())?;

        Ok(Self {
            query,
            _phantom: std::marker::PhantomData,
        })
    }
    fn with_precomputed(
        source: &str,
        language: &tree_sitter::Language,
        precomputeds: impl ArrayStr,
    ) -> Result<Self, tree_sitter::QueryError> {
        let query = crate::Query::with_precomputed(source, language.clone(), precomputeds)?.1;

        Ok(Self {
            query,
            _phantom: std::marker::PhantomData,
        })
    }
}

impl<HAST> Debug for QueryMatcher<HAST> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.query)
    }
}

#[cfg(feature = "tsg")]
impl<HAST> tree_sitter_graph::GenQuery for QueryMatcher<HAST>
where
    HAST: 'static + hyperast::types::HyperASTShared + for<'tree> hyperast::types::HyperAST<'tree>,
    for<'tree> <HAST as HyperAST<'tree>>::TS: hyperast::types::RoleStore,
    for<'tree> <<HAST as HyperAST<'tree>>::TS as hyperast::types::RoleStore>::IdF:
        From<u16> + Into<u16>,
    for<'tree> <HAST as HyperAST<'tree>>::T: WithSerialization + WithStats + WithRoles,
    <HAST as HyperASTShared>::IdN: std::fmt::Debug + Copy + std::hash::Hash,
    <HAST as HyperASTShared>::Idx: Copy + std::hash::Hash,
    for<'tree> <HAST as HyperAST<'tree>>::T: WithPrecompQueries,
{
    type Lang = tree_sitter::Language;

    type Ext = ExtendingStringQuery<Self, Self::Lang>;

    fn pattern_count(&self) -> usize {
        self.query.enabled_pattern_count()
    }

    fn capture_index_for_name(&self, name: &str) -> Option<u32> {
        // dbg!(&self.capture_names);
        self.query
            .capture_index_for_name(name)
            .map(|x| x.to_usize() as u32)
    }

    fn capture_quantifiers(
        &self,
        index: usize,
    ) -> impl std::ops::Index<usize, Output = tree_sitter::CaptureQuantifier> {
        // let index = self.query.enabled_pattern_map[index] as usize;
        // assert_ne!(index, u16::MAX as usize);
        let index = self.query.enabled_pattern_map.iter().position(|x|*x as usize==index).unwrap();
        self.query.capture_quantifiers(index)
    }

    fn capture_names(&self) -> &[&str] {
        todo!()
    }

    fn check(
        file: &mut tree_sitter_graph::ast::File<Self>,
    ) -> Result<(), tree_sitter_graph::checker::CheckError>
    where
        Self: Sized,
    {
        file.check2()
    }

    type Node<'tree> = Node<'tree, HAST>;

    type Cursor = Vec<u16>;

    type Match<'cursor, 'tree: 'cursor>
        = self::MyQMatch<'cursor, 'tree, HAST>
    where
        Self: 'cursor;

    type Matches<'query, 'cursor: 'query, 'tree: 'cursor>
        = self::MyQMatches<
        'query,
        'cursor,
        'tree,
        crate::QueryCursor<'query, Self::Node<'tree>, Self::Node<'tree>>,
        HAST,
    >
    where
        Self: 'tree,
        Self: 'query,
        Self: 'cursor;

    type I = u32;

    fn matches<'query, 'cursor: 'query, 'tree: 'cursor>(
        &'query self,
        cursor: &'cursor mut Self::Cursor,
        node: &Self::Node<'tree>,
    ) -> Self::Matches<'query, 'cursor, 'tree> {
        let matchs = self.query.matches(node.clone());
        // let matchs = self.query.matches_immediate(node.clone());
        let node = node.clone();
        self::MyQMatches {
            q: self,
            cursor,
            matchs,
            node,
        }
    }
}

pub struct ExtendingStringQuery<Q = tree_sitter::Query, L = tree_sitter::Language> {
    pub(crate) query: Option<Q>,
    pub(crate) acc: String,
    pub(crate) precomputeds: Option<Box<dyn ArrayStr>>,
    pub(crate) language: Option<L>,
}

impl<Q, L> ExtendingStringQuery<Q, L> {
    fn empty() -> Self {
        Self {
            query: Default::default(),
            acc: Default::default(),
            precomputeds: Default::default(),
            language: None,
        }
    }
    pub fn new(language: L, precomputeds: Box<dyn ArrayStr>, capacity: usize) -> Self {
        Self {
            acc: String::with_capacity(capacity),
            precomputeds: Some(precomputeds),
            language: Some(language),
            ..Self::empty()
        }
    }
}

#[cfg(feature = "tsg")]
impl<HAST> tree_sitter_graph::ExtendedableQuery
    for ExtendingStringQuery<QueryMatcher<HAST>, tree_sitter::Language>
where
    HAST: 'static + hyperast::types::HyperASTShared + for<'tree> hyperast::types::HyperAST<'tree>,
    for<'tree> <HAST as HyperAST<'tree>>::T: WithSerialization + WithStats + WithRoles,
    <HAST as HyperASTShared>::IdN: std::fmt::Debug + Copy + std::hash::Hash,
    <HAST as HyperASTShared>::Idx: Copy + std::hash::Hash,
    for<'tree> <HAST as HyperAST<'tree>>::TS: hyperast::types::RoleStore,
    for<'tree> <<HAST as HyperAST<'tree>>::TS as hyperast::types::RoleStore>::IdF:
        From<u16> + Into<u16>,
    for<'tree> <HAST as HyperAST<'tree>>::T: WithPrecompQueries,
{
    type Query = QueryMatcher<HAST>;
    type Lang = tree_sitter::Language;

    fn as_ref(&self) -> Option<&Self::Query> {
        self.query.as_ref()
    }

    fn with_capacity(capacity: usize) -> Self {
        let acc = String::with_capacity(capacity);
        Self {
            acc,
            ..Self::empty()
        }
    }

    fn make_query(
        &mut self,
        language: &Self::Lang,
        source: &str,
    ) -> Result<Self::Query, tree_sitter::QueryError> {
        if let Some(l) = &self.language {
            // this impl cannot accept different languages
            assert_eq!(language, l);
        }
        self.acc += source;
        self.acc += "\n";
        dbg!(source);
        // QueryMatcher::new(source, language)
        if let Some(precomputeds) = &self.precomputeds {
            QueryMatcher::with_precomputed(source, language, precomputeds.as_ref())
        } else {
            QueryMatcher::new(source, language)
        }
    }

    fn make_main_query(&self, language: &Self::Lang) -> Self::Query {
        if let Some(l) = &self.language {
            // this impl cannot accept different languages
            // Moreover, given the existance of a main query, having multiple languages should be impossible.
            assert_eq!(language, l);
        }
        // QueryMatcher::new(&self.acc, language).unwrap()
        if let Some(precomputeds) = &self.precomputeds {
            QueryMatcher::with_precomputed(&self.acc, language, precomputeds.as_ref()).unwrap()
        } else {
            QueryMatcher::new(&self.acc, language).unwrap()
        }
    }
}

#[cfg(feature = "tsg")]
impl<'tree, HAST: hyperast::types::HyperAST<'tree>> tree_sitter_graph::graph::SyntaxNode
    for Node<'tree, HAST>
where
    HAST::IdN: Copy + std::hash::Hash + Debug,
    HAST::Idx: Copy + std::hash::Hash,
    HAST::T: WithSerialization + WithStats,
{
    fn id(&self) -> usize {
        use std::hash::Hash;
        use std::hash::Hasher;
        let mut hasher = std::hash::DefaultHasher::new();
        self.0.pos.hash(&mut hasher);
        hasher.finish() as usize
    }

    fn kind(&self) -> &'static str {
        use hyperast::position::position_accessors::SolvedPosition;
        let n = self.0.pos.node();
        let n = self.0.stores.resolve_type(&n);
        use hyperast::types::HyperType;
        n.as_static_str()
    }

    fn start_position(&self) -> tree_sitter::Point {
        let conv =
            hyperast::position::PositionConverter::new(&self.0.pos).with_stores(self.0.stores);
        let pos: hyperast::position::row_col::RowCol<usize> =
            conv.compute_pos_post_order::<_, hyperast::position::row_col::RowCol<usize>, _>();
        // use hyperast::position::computing_offset_bottom_up::extract_position_it;
        // let p = extract_position_it(self.stores, self.pos.iter());
        tree_sitter::Point {
            row: pos.row() as usize, //p.range().start,
            column: pos.col() as usize,
        }
    }

    fn end_position(&self) -> tree_sitter::Point {
        todo!()
    }

    fn byte_range(&self) -> std::ops::Range<usize> {
        todo!()
    }

    fn range(&self) -> tree_sitter::Range {
        let r = self.byte_range();
        tree_sitter::Range {
            start_byte: r.start,
            end_byte: r.end,
            start_point: self.start_position(),
            end_point: self.end_position(),
        }
    }

    fn text(&self) -> String {
        use hyperast::position::position_accessors::SolvedPosition;
        hyperast::nodes::TextSerializer::new(self.0.stores, self.0.pos.node()).to_string()
    }

    fn named_child_count(&self) -> usize {
        todo!()
    }

    fn parent(&self) -> Option<Self>
    where
        Self: Sized,
    {
        todo!()
    }
}

#[cfg(feature = "tsg")]
impl<'tree, HAST: HyperAST<'tree>> tree_sitter_graph::graph::SyntaxNodeExt for Node<'tree, HAST>
where
    <HAST as HyperASTShared>::IdN: Copy + std::hash::Hash + Debug,
    HAST::Idx: Copy + std::hash::Hash,
    HAST::T: WithSerialization + WithStats,
    HAST::IdN: std::fmt::Debug + Copy,
    HAST::TS: RoleStore,
    HAST::T: WithRoles,
    HAST::T: WithPrecompQueries,
{
    type Cursor = Vec<Self>;

    fn walk(&self) -> Self::Cursor {
        todo!()
    }

    fn named_children<'cursor>(
        &self,
        _cursor: &'cursor mut Self::Cursor,
    ) -> impl ExactSizeIterator<Item = Self> + 'cursor
    where
        'tree: 'cursor,
    {
        todo!();
        vec![].iter().cloned()
    }

    type QM<'cursor>
        = MyQMatch<'cursor, 'tree, HAST>
    where
        Self: 'cursor;
}

pub struct MyQMatch<'cursor, 'tree, HAST: HyperAST<'tree>> {
    stores: &'tree HAST,
    b: &'cursor (),
    qm: crate::QueryMatch<Node<'tree, HAST>>,
    i: u16
}

#[cfg(feature = "tsg")]
impl<'cursor, 'tree, HAST: hyperast::types::HyperAST<'tree>> tree_sitter_graph::graph::QMatch
    for MyQMatch<'cursor, 'tree, HAST>
where
    HAST::IdN: std::fmt::Debug + Copy,
    HAST::TS: RoleStore,
    HAST::T: WithRoles,
    HAST::T: WithPrecompQueries,
{
    type I = u32;

    type Item = Node<'tree, HAST>;

    fn nodes_for_capture_index(&self, index: Self::I) -> impl Iterator<Item = Self::Item> + '_ {
        self.qm
            .nodes_for_capture_index(CaptureId::new(index))
            .cloned()
    }

    fn pattern_index(&self) -> usize {
        // self.qm.pattern_index.to_usize()
        self.i as usize
    }
}

pub struct MyQMatches<'query, 'cursor: 'query, 'tree: 'cursor, It, HAST: HyperASTShared> {
    q: &'query QueryMatcher<HAST>,
    cursor: &'cursor mut Vec<u16>,
    matchs: It,
    node: Node<'tree, HAST>,
}

impl<'query, 'cursor: 'query, 'tree: 'cursor, It, HAST: HyperAST<'tree>> Iterator
    for MyQMatches<'query, 'cursor, 'tree, It, HAST>
where
    It: Iterator<Item = crate::QueryMatch<Node<'tree, HAST>>>,
{
    type Item = self::MyQMatch<'cursor, 'tree, HAST>;

    fn next(&mut self) -> Option<Self::Item> {
        let qm = self.matchs.next()?;
        let stores = self.node.0.stores;
        let i = self.q.query.enabled_pattern_index(qm.pattern_index).unwrap();
        Some(self::MyQMatch {
            stores,
            b: &&(),
            qm,
            i,
        })
    }
}
