use hyperast::tree_gen::NoOpMore;
use tree_sitter::Parser;

use crate::TStore;
use crate::legion::tree_sitter_parse;

type CTreeGen<'store, 'cache, HAST, Acc> =
    crate::legion::CTreeGen<'store, 'cache, TStore, NoOpMore<HAST, Acc>, true>;
type SimpleStores = hyperast::store::SimpleStores<TStore>;

static EX: &str = r#"
void read_string(char *buf) {
    scanf("%s ", buf);
}"#;

#[test]
pub(crate) fn cpp_tree_sitter_simple() {
    let mut parser = Parser::new();

    {
        parser.set_language(&crate::language()).unwrap();
    }

    let text = { EX.as_bytes() };
    let tree = parser.parse(text, None).unwrap();
    println!("{}", tree.root_node().to_sexp());
}

#[test]
pub(crate) fn c_simple_test() {
    let text = EX;
    let tree = tree_sitter_parse(text.as_bytes());
    println!("{:#?}", tree.root_node().to_sexp());
    let mut stores = SimpleStores::default();
    let mut md_cache = Default::default();
    let mut tree_gen = CTreeGen::new(&mut stores, &mut md_cache);
    let x = tree_gen.generate_file(b"", text.as_bytes(), tree.walk());
    let x = x.local.compressed_node;
    println!("{}", hyperast::nodes::SyntaxSerializer::new(&stores, x));
    println!("{}", hyperast::nodes::SexpSerializer::new(&stores, x));
    println!("{}", hyperast::nodes::TextSerializer::new(&stores, x));
    assert_eq!(
        text,
        hyperast::nodes::TextSerializer::new(&stores, x).to_string()
    );
}

// https://github.com/tursodatabase/libsql/blob/6f451a1fabacbcbc9960b232b4c1605a5021979b/libsql-sqlite3/src/shell.c.in
#[test]
pub(crate) fn c_sql_shell_main_begining_test() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tests/shell_main_begining.c");
    let text = std::fs::read_to_string(&path).unwrap();
    let tree = tree_sitter_parse(text.as_bytes());
    println!("{:#?}", tree.root_node().to_sexp());
    let mut stores = SimpleStores::default();
    let mut tree_gen = crate::legion_ts_simp::CTreeGen::bare(&mut stores);
    let x = tree_gen.generate_file(b"", text.as_bytes(), tree.walk());
    let x = x.local.compressed_node;
    println!(
        "{}",
        hyperast::nodes::IndentedSerializer::<_, _, hyperast::nodes::Json, true>::new(&stores, x)
    );
    pretty_assertions::assert_eq!(
        text,
        hyperast::nodes::TextSerializer::new(&stores, x).to_string()
    );
}

#[test]
pub(crate) fn c_preproc_space_define_test() {
    let text = r##"#  define data shellState"##;
    // # define session_close_all(X,Y)
    let tree = tree_sitter_parse(text.as_bytes());
    println!("{:#?}", tree.root_node().to_sexp());
    let mut stores = SimpleStores::default();
    let mut tree_gen = crate::legion_ts_simp::CTreeGen::bare(&mut stores);
    let x = tree_gen.generate_file(b"", text.as_bytes(), tree.walk());
    let x = x.local.compressed_node;
    println!(
        "{}",
        hyperast::nodes::IndentedSerializer::<_, _, hyperast::nodes::Json, true>::new(&stores, x)
    );
    pretty_assertions::assert_eq!(
        text,
        hyperast::nodes::TextSerializer::new(&stores, x).to_string()
    );
}

/// A bare list of bytes, as the FPGA bitstreams of u-boot (`board/**/fpgadata.c`, included
/// inside an array initializer elsewhere): not valid C on its own, its error node gets more than
/// 10,000 children, so the generator splits them into repeat nodes (`zipped_ts_extra::_post`).
fn bare_byte_list(indent: &str) -> String {
    (0..6_600)
        .map(|i: usize| {
            let start = if i % 16 == 0 { indent } else { "" };
            let end = if i % 16 == 15 { "\n" } else { "" };
            format!("{}0x{:02x},{}", start, (i * 37) % 256, end)
        })
        .collect()
}

fn assert_round_trip(text: &str) {
    let tree = tree_sitter_parse(text.as_bytes());
    let mut stores = SimpleStores::default();
    let mut tree_gen = crate::legion_ts_simp::CTreeGen::bare(&mut stores);
    let x = tree_gen.generate_file(b"", text.as_bytes(), tree.walk());
    let x = x.local.compressed_node;
    pretty_assertions::assert_eq!(
        text,
        hyperast::nodes::TextSerializer::new(&stores, x).to_string()
    );
}

/// The split kept the parent's start but moved its padding start to the current position: when
/// the parent did not start at byte 0 (here a leading space), `_post` met padding > start and
/// panicked (`unreachable!`, "should normally handle all cases").
#[test]
pub(crate) fn c_many_children_after_leading_space_test() {
    assert_round_trip(&bare_byte_list(" "));
}

/// The same without the leading space (the case the "repeat trick" exception of `_post` covered).
#[test]
pub(crate) fn c_many_children_test() {
    assert_round_trip(&bare_byte_list(""));
}

use crate::Type;
use hyperast::nodes::{SyntaxSerializer, TextSerializer};
use hyperast::tree_gen;
use hyperast::tree_gen::utils_ts;

#[test]
fn medium() {
    type CGen<'store, 'b, More> = tree_gen::zipped_ts::TsTreeGen<'store, 'b, TStore, More, true>;
    let mut stores = Default::default();
    let mut md_cache = Default::default();
    let mut r#gen = CGen::new(&mut stores, &mut md_cache);
    let text = EX;
    let tree = utils_ts::tree_sitter_parse(text.as_bytes(), &crate::language());
    eprintln!("{}", tree.root_node().to_sexp());
    let name = b"";
    let f = r#gen.generate_file(name, text.as_bytes(), tree.walk());
    assert_eq!(f.local._ty, Type::TranslationUnit);
    let id = f.local.compressed_node;
    println!("{}", SyntaxSerializer::new(&stores, id));
    println!("\n{}", TextSerializer::new(&stores, id));
    dbg!(&f.local.metrics);
    assert_eq!(text, TextSerializer::new(&stores, id).to_string());
}

#[test]
fn medium_extra_pattern_precomp() {
    use hyperast_tsquery::{PreparedQuerying, Query};
    use tree_gen::zipped_ts_extra::Acc;

    use crate::legion_ts_simp::CTreeGen;
    use hyperast::tree_gen::extra_pattern_precomp::PatternPrecompExtra as Precomp;

    let lang = crate::language();
    let name = b"";
    let text = EX;
    let query = "(identifier)\n(type_identifier)\n(primitive_type)";
    let precomp = [
        "(identifier)",
        "(type_identifier)",
        "(parameter_declaration (type_identifier) (identifier))",
        "(primitive_type)",
    ]
    .as_slice();

    let (precomp, _q) = Query::with_precomputed(query, lang, precomp) //
        .unwrap_or_else(|e| panic!("\n{e}"));
    let more = PreparedQuerying::<_, TStore, Acc<Type>>::from(&precomp);
    let mut extra = Precomp::<_, Acc<Type>, _>::with_cache(more, Default::default());

    let mut stores = Default::default();
    let mut r#gen = CTreeGen::new(&mut stores, &mut extra);
    let tree = utils_ts::tree_sitter_parse(text.as_bytes(), &crate::language());
    eprintln!("{}", tree.root_node().to_sexp());

    let f = r#gen.generate_file(name, text.as_bytes(), tree.walk());
    let id = f.local.compressed_node;

    println!("{}", SyntaxSerializer::new(&stores, id));
    type SerWithRoles<'a, IdN, HAST> =
        hyperast::nodes::SimpleSerializer<'a, IdN, HAST, true, false, false, false, true>;
    println!("{}", SerWithRoles::new(&stores, id));
    println!("\n{}", TextSerializer::new(&stores, id));

    assert_eq!(text, TextSerializer::new(&stores, id).to_string());
    dbg!(&f.local.metrics);
    dbg!(&f.extra);
    assert_eq!(0b1001, f.extra.0)
}

#[test]
fn not_simple() {
    let mut stores = SimpleStores::default();
    let mut md_cache = Default::default();
    let mut r#gen = crate::legion::CTreeGen::new(&mut stores, &mut md_cache);
    let text = EX;
    let tree = utils_ts::tree_sitter_parse(text.as_bytes(), &crate::language());
    eprintln!("{}", tree.root_node().to_sexp());
    let name = b"";
    let f = r#gen.generate_file(name, text.as_bytes(), tree.walk());
    let id = f.local.compressed_node;
    println!("{}", SyntaxSerializer::new(&stores, id));
    println!("\n{}", TextSerializer::new(&stores, id));
    dbg!(&f.local.metrics);
    assert_eq!(text, TextSerializer::new(&stores, id).to_string());
}

#[test]
pub(crate) fn issue_npy_nditer_test() {
    let text = CODE_NPY_NDITER_CONSTR;
    let text = text.as_bytes();
    let tree = tree_sitter_parse(text);
    println!("{:#?}", tree.root_node().to_sexp());
    let mut stores = SimpleStores::default();
    use crate::legion_ts_simp::CTreeGen;
    let mut tree_gen = CTreeGen::bare(&mut stores);
    // let mut md_cache = Default::default();
    // let mut tree_gen = CTreeGen::new(&mut stores, &mut md_cache);
    let x = tree_gen.generate_file(b"", text, tree.walk()).node.local;
    println!(
        "{}",
        hyperast::nodes::SyntaxSerializer::new(&stores, x.compressed_node)
    );
    // println!("{}", tree.root_node().to_sexp());
}

// https://github.com/numpy/numpy/blob/main/numpy/_core/src/multiarray/nditer_constr.c
pub(crate) const CODE_NPY_NDITER_CONSTR: &str = r#"NPY_NO_EXPORT NpyIter *
NpyIter_AdvancedNew()
{
    NIT_ITERINDEX(iter) = 0;
    memset(NIT_BASEOFFSETS(iter), 0, (nop+1)*NPY_SIZEOF_INTP);
}"#;

/// The matches of a query on a C file with HyperAST (the generator of the git processor).
fn hyperast_matches(query: &str, text: &str) -> usize {
    let query = hyperast_tsquery::Query::new(query, crate::language()).unwrap();
    let tree = tree_sitter_parse(text.as_bytes());
    let mut stores = SimpleStores::default();
    let mut tree_gen = crate::legion_ts_simp::CTreeGen::bare(&mut stores);
    let file = tree_gen
        .generate_file(b"", text.as_bytes(), tree.walk())
        .local
        .compressed_node;
    let pos = hyperast::position::StructuralPosition::new(file);
    let cursor = hyperast_tsquery::hyperast_cursor::TreeCursor::new(&stores, pos);
    let mut matches = query.matches(cursor);
    let mut count = 0;
    while matches.next().is_some() {
        count += 1;
    }
    count
}

/// The matches of a query on a C file with Tree-sitter's own query cursor.
fn tree_sitter_matches(query: &str, text: &str) -> usize {
    use streaming_iterator::StreamingIterator;
    let query = tree_sitter::Query::new(&crate::language(), query).unwrap();
    let tree = tree_sitter_parse(text.as_bytes());
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), text.as_bytes());
    let mut count = 0;
    while matches.next().is_some() {
        count += 1;
    }
    count
}

/// A query on a node whose children are wrapped in hidden supertypes (an initializer list: each
/// element is an `expression`) is not quadratic in the width of the node: `current_status` kept
/// walking the later siblings after a supertype (fixed by a `break`, 224861a). On 4,000 elements:
/// 7.3 s before the fix, 39 ms after (release; 0.4 s in debug), 1.8 ms with Tree-sitter. The
/// query matches nowhere, as most queries on most files.
#[test]
fn c_query_on_wide_nodes_test() {
    const NO_MATCH: &str = "(if_statement condition: (parenthesized_expression \
        (binary_expression left: (identifier) right: (number_literal)))) @if";
    let initializer = |n: usize| {
        let elements: Vec<String> = (0..n).map(|i| i.to_string()).collect();
        format!("int table[] = {{{}}};\n", elements.join(", "))
    };
    // the sibling walks of `current_status` are not counted by the cursor (`status_count`): the
    // time is the measure, with a wide margin
    let text = initializer(4_000);
    let start = std::time::Instant::now();
    let matches = hyperast_matches(NO_MATCH, &text);
    let elapsed = start.elapsed();
    assert_eq!(matches, 0);
    assert!(elapsed < std::time::Duration::from_secs(2), "{:?} for 4,000 elements", elapsed);
}

/// A C file without parse errors, with most kinds of declarations, statements and expressions.
const VARIED: &str = r#"#include <stdio.h>
#define MAX(a, b) ((a) > (b) ? (a) : (b))

struct point { int x; int y; };
typedef enum { RED, GREEN = 2 } color;
union value { int i; float f; };
static const char *names[] = {"a\tb", "c\n", "plain"};

static int sum(const int *values, size_t n) {
    int total = 0;
    for (size_t i = 0; i < n; i++) {
        total += values[i];
    }
    return total;
}

int main(int argc, char **argv) {
    struct point p = {.x = 1, .y = 2};
    color c = GREEN;
    int values[] = {1, 2, 3};
    char *s = argc > 1 ? argv[1] : "none";
    while (p.x < 10) p.x++;
    do { p.y--; } while (p.y > 0);
    switch (c) {
    case RED: puts("red"); break;
    default: break;
    }
    if (sum(values, sizeof values / sizeof *values) != 6 && !s) {
        goto end;
    }
    printf("%d %s %c\n", MAX(p.x, p.y), names[0], 'z');
end:
    return (int) 0.5f;
}
"#;

/// HyperAST and Tree-sitter count the same matches for every named node type of a C file, and
/// for the children of string literals. On a file without parse errors: HyperAST leaves out the
/// empty nodes of Tree-sitter's error recovery (e.g. `(expression_statement (MISSING ";"))` after
/// a label that ends a block), which Tree-sitter's queries match.
#[test]
fn c_same_matches_as_tree_sitter_test() {
    let text = VARIED.to_string();
    let tree = tree_sitter_parse(text.as_bytes());
    assert!(!tree.root_node().has_error(), "{}", tree.root_node().to_sexp());
    let mut kinds = std::collections::BTreeSet::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.is_named() && !node.is_error() {
            kinds.insert(node.kind().to_string());
        }
        let mut walk = node.walk();
        stack.extend(node.children(&mut walk));
    }
    let mut queries: Vec<String> = kinds.iter().map(|kind| format!("({kind}) @c")).collect();
    queries.push("(string_literal (string_content)) @c".to_string());
    queries.push("(string_literal (escape_sequence)) @c".to_string());
    let differ: Vec<String> = queries
        .iter()
        .filter_map(|query| {
            let hyperast = hyperast_matches(query, &text);
            let tree_sitter = tree_sitter_matches(query, &text);
            (hyperast != tree_sitter)
                .then(|| format!("{query}: HyperAST {hyperast}, Tree-sitter {tree_sitter}"))
        })
        .collect();
    assert!(queries.len() > 60, "too few node types: {}", queries.len());
    assert!(differ.is_empty(), "{} of {} queries differ:\n{}", differ.len(), queries.len(), differ.join("\n"));
}
