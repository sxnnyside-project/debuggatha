//! The layer model, the upward dependencies it exposes, and the cycles of the
//! dependency graph. The cases are the specification; the corpus test compares with the TypeScript
//! engine and explains every difference.
#![allow(clippy::expect_used, clippy::unwrap_used)]
// a failed expect is a failed test
#![allow(clippy::too_many_lines)] // tables of cases
// `assert!(x.is_empty())` is what these cases mean.
#![allow(clippy::assert_is_empty)]

use debuggatha_engine::analysis::boundaries::{Manifest, ModuleBoundary, ModuleKind};
use debuggatha_engine::analysis::{
    DependencyEdge, DependencyGraph, EdgeKind, Evidence, Impact, LayerName, build_layer_model,
    detect_cycles, detect_layer_violations,
};

fn boundary(id: &str, name: Option<&str>) -> ModuleBoundary {
    ModuleBoundary {
        id: id.into(),
        kind: ModuleKind::Package,
        name: name.map(Into::into),
        manifest: Manifest::None,
        files: vec![],
    }
}

fn layer_of(id: &str, name: Option<&str>) -> LayerName {
    let b = boundary(id, name);
    build_layer_model(std::slice::from_ref(&b)).assignments[id]
}

#[test]
fn a_layer_word_must_be_a_whole_word_of_the_name() {
    use LayerName::{Adapter, Application, Domain, Infrastructure, Presentation, Unknown};
    let cases: &[(&str, Option<&str>, LayerName)] = &[
        ("packages/x", Some("@x/domain-users"), Domain),
        ("packages/x", Some("@x/adapters-db"), Adapter),
        ("src/Ordering.Domain", None, Domain),
        ("src/Ordering.Infrastructure", None, Infrastructure),
        ("src/core", None, Domain),
        ("src/models", None, Domain),
        ("src/entities", None, Domain),
        ("src/app", None, Application),
        ("src/services", None, Application),
        ("src/use-cases", None, Application),
        ("src/usecases", None, Application),
        ("src/repositories", None, Adapter),
        ("src/gateways", None, Adapter),
        ("src/persistence", None, Infrastructure),
        ("src/data", None, Infrastructure),
        ("src/ui", None, Presentation),
        ("src/Views", None, Presentation),
        ("src/pages", None, Presentation),
        ("src/components", None, Presentation),
        ("src/web", None, Presentation),
        ("packages/animoria-ui", Some("@animoria/ui"), Presentation),
        ("crates/x", Some("obfusku-core"), Domain),
        ("crates/x", Some("my_core_lib"), Domain),
        // A layer word inside a longer word is not that word.
        ("src/ClientApp", None, Unknown),
        ("src/WebApp", None, Unknown),
        ("src/Webhooks.API", None, Unknown),
        ("src/WebhookClient", None, Unknown),
        ("src/tui", None, Unknown),
        ("src/metadata", None, Unknown),
        ("src/webapp", None, Unknown),
        ("src/corelib", None, Unknown),
        ("src/whatever", None, Unknown),
        // Two layers in one name: the name does not say which.
        ("src/domain-adapters", None, Unknown),
        ("packages/x", Some("@x/ui-core"), Unknown),
    ];
    for (id, name, want) in cases {
        assert_eq!(layer_of(id, *name), *want, "{id} / {name:?}");
    }
}

#[test]
fn the_name_is_read_before_the_directory() {
    // A package called "billing" under `packages/domain` is not placed by its directory.
    assert_eq!(
        layer_of("packages/domain", Some("billing")),
        LayerName::Unknown
    );
    assert_eq!(layer_of("packages/domain", None), LayerName::Domain);
}

fn ev(file: &str, detail: &str) -> Evidence {
    Evidence {
        file: file.into(),
        detail: detail.into(),
        line: None,
    }
}

fn edge(from: &str, to: &str, kind: EdgeKind, evidence: Vec<Evidence>) -> DependencyEdge {
    DependencyEdge {
        from: from.into(),
        to: to.into(),
        kind,
        evidence,
    }
}

fn graph(nodes: &[&str], edges: Vec<DependencyEdge>) -> DependencyGraph {
    DependencyGraph {
        nodes: nodes.iter().map(|n| (*n).to_owned()).collect(),
        edges,
        ..DependencyGraph::default()
    }
}

fn model(pairs: &[(&str, LayerName)]) -> debuggatha_engine::analysis::LayerModel {
    let mut m = debuggatha_engine::analysis::LayerModel::default();
    for (id, layer) in pairs {
        m.assignments.insert((*id).to_owned(), *layer);
    }
    m
}

#[test]
fn an_inner_layer_depending_on_an_outer_one_is_a_violation() {
    use LayerName::{Adapter, Application, Domain, Infrastructure, Presentation, Unknown};
    let layers = model(&[
        ("d", Domain),
        ("a", Application),
        ("ad", Adapter),
        ("i", Infrastructure),
        ("p", Presentation),
        ("u", Unknown),
    ]);
    let g = graph(
        &["a", "ad", "d", "i", "p", "u"],
        vec![
            // downward or sideways: fine
            edge("p", "d", EdgeKind::Import, vec![]),
            edge("a", "d", EdgeKind::Import, vec![]),
            edge("ad", "a", EdgeKind::Import, vec![]),
            edge("i", "ad", EdgeKind::Import, vec![]),
            // unknown on either end: never an accusation
            edge("d", "u", EdgeKind::Import, vec![]),
            edge("u", "p", EdgeKind::Import, vec![]),
            // upward
            edge(
                "a",
                "i",
                EdgeKind::Import,
                vec![ev("a/x.ts", "imports \"../i\"")],
            ),
            edge(
                "d",
                "p",
                EdgeKind::Import,
                vec![ev("d/x.ts", "imports \"../p\"")],
            ),
        ],
    );
    let got: Vec<_> = detect_layer_violations(&g, &layers)
        .into_iter()
        .map(|v| (v.from, v.to, v.from_layer, v.to_layer))
        .collect();
    assert_eq!(
        got,
        vec![
            ("a".to_owned(), "i".to_owned(), Application, Infrastructure),
            ("d".to_owned(), "p".to_owned(), Domain, Presentation),
        ]
    );
}

#[test]
fn a_violation_explains_both_sides_and_merges_a_declared_and_an_import_edge() {
    let layers = model(&[
        ("core", LayerName::Domain),
        ("web", LayerName::Presentation),
    ]);
    let g = graph(
        &["core", "web"],
        vec![
            edge(
                "core",
                "web",
                EdgeKind::Declared,
                vec![ev("package.json", "declares dependency on \"web\"")],
            ),
            edge(
                "core",
                "web",
                EdgeKind::Import,
                vec![ev("core/a.ts", "imports \"web\"")],
            ),
        ],
    );
    let v = detect_layer_violations(&g, &layers);
    assert_eq!(v.len(), 1);
    assert_eq!(
        v[0].explanation,
        "\"core\" (layer \"domain\") depends on \"web\" (layer \"presentation\"), but \"presentation\" is conventionally outer relative to \"domain\" — this is an upward dependency against the assumed layer order (domain -> application -> adapter -> infrastructure -> presentation)."
    );
    let files: Vec<_> = v[0].evidence.iter().map(|e| e.file.as_str()).collect();
    assert_eq!(files, ["package.json", "core/a.ts"]);
}

#[test]
fn a_boundary_the_model_does_not_know_is_unknown_not_a_violation() {
    let g = graph(&["a", "b"], vec![edge("a", "b", EdgeKind::Import, vec![])]);
    assert!(detect_layer_violations(&g, &model(&[("a", LayerName::Domain)])).is_empty());
}

fn cycle_graph(edges: &[(&str, &str)]) -> DependencyGraph {
    let mut nodes: Vec<&str> = edges.iter().flat_map(|(a, b)| [*a, *b]).collect();
    nodes.sort_unstable();
    nodes.dedup();
    let mut sorted = edges.to_vec();
    sorted.sort_unstable();
    graph(
        &nodes,
        sorted
            .into_iter()
            .map(|(a, b)| edge(a, b, EdgeKind::Import, vec![]))
            .collect(),
    )
}

fn shape(g: &DependencyGraph) -> Vec<(Vec<String>, Vec<String>, Impact)> {
    detect_cycles(g)
        .into_iter()
        .map(|c| (c.modules, c.shortest_cycle, c.impact))
        .collect()
}

fn v(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn a_cycle_is_a_strongly_connected_component_and_its_size_sets_the_impact() {
    assert_eq!(
        shape(&cycle_graph(&[("a", "b"), ("b", "a")])),
        vec![(v(&["a", "b"]), v(&["a", "b", "a"]), Impact::Low)]
    );
    assert_eq!(
        shape(&cycle_graph(&[("a", "b"), ("b", "c"), ("c", "a")])),
        vec![(
            v(&["a", "b", "c"]),
            v(&["a", "b", "c", "a"]),
            Impact::Medium
        )]
    );
    assert_eq!(
        shape(&cycle_graph(&[
            ("a", "b"),
            ("b", "c"),
            ("c", "d"),
            ("d", "a")
        ])),
        vec![(
            v(&["a", "b", "c", "d"]),
            v(&["a", "b", "c", "d", "a"]),
            Impact::High
        )]
    );
}

#[test]
fn nothing_is_reported_for_a_graph_without_a_cycle() {
    assert!(detect_cycles(&cycle_graph(&[("a", "b"), ("b", "c"), ("a", "c")])).is_empty());
    assert!(detect_cycles(&DependencyGraph::default()).is_empty());
    // A module that depends on itself is not a graph edge, and a lone node is no cycle.
    assert!(detect_cycles(&graph(&["a"], vec![])).is_empty());
}

#[test]
fn overlapping_loops_are_one_cycle_and_disjoint_ones_are_two() {
    // a-b-c with a chord and a second loop through the same nodes: one component.
    let one = cycle_graph(&[("a", "b"), ("b", "c"), ("c", "a"), ("c", "b"), ("a", "c")]);
    let got = shape(&one);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, v(&["a", "b", "c"]));
    // The shortest way back to `a` is the chord `a -> c -> a`.
    assert_eq!(got[0].1, v(&["a", "c", "a"]));

    let two = cycle_graph(&[("x", "y"), ("y", "x"), ("a", "b"), ("b", "a"), ("a", "x")]);
    let names: Vec<_> = shape(&two).into_iter().map(|c| c.0).collect();
    assert_eq!(names, vec![v(&["a", "b"]), v(&["x", "y"])]);
}

#[test]
fn a_node_outside_the_component_is_not_in_the_cycle() {
    // `t` is reached from the cycle and leads nowhere; `s` leads into it.
    let g = cycle_graph(&[("s", "a"), ("a", "b"), ("b", "a"), ("b", "t")]);
    assert_eq!(
        shape(&g),
        vec![(v(&["a", "b"]), v(&["a", "b", "a"]), Impact::Low)]
    );
}

#[test]
fn a_very_long_chain_does_not_overflow_the_stack() {
    let n = 30_000;
    let names: Vec<String> = (0..n).map(|i| format!("m{i:06}")).collect();
    let mut edges: Vec<DependencyEdge> = (0..n - 1)
        .map(|i| edge(&names[i], &names[i + 1], EdgeKind::Import, vec![]))
        .collect();
    edges.push(edge(&names[n - 1], &names[0], EdgeKind::Import, vec![]));
    edges.sort_by(|a, b| a.from.cmp(&b.from));
    let g = DependencyGraph {
        nodes: names.clone(),
        edges,
        ..DependencyGraph::default()
    };
    let cycles = detect_cycles(&g);
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0].modules.len(), n);
    assert_eq!(cycles[0].impact, Impact::High);
}

#[test]
fn cycles_come_out_in_a_stable_order() {
    let g = cycle_graph(&[("z", "y"), ("y", "z"), ("b", "a"), ("a", "b")]);
    let names: Vec<_> = shape(&g).into_iter().map(|c| c.0).collect();
    assert_eq!(names, vec![v(&["a", "b"]), v(&["y", "z"])]);
}

#[test]
fn two_boundaries_of_one_layer_may_depend_on_each_other() {
    let layers = model(&[("d1", LayerName::Domain), ("d2", LayerName::Domain)]);
    let g = graph(
        &["d1", "d2"],
        vec![edge("d1", "d2", EdgeKind::Import, vec![])],
    );
    assert!(detect_layer_violations(&g, &layers).is_empty());
}

#[test]
fn modules_are_sorted_whatever_order_the_search_found_them() {
    // The search enters at `a`, goes to `c`, then `b`: the stack order is not the sorted order.
    let g = cycle_graph(&[("a", "c"), ("c", "b"), ("b", "a")]);
    assert_eq!(
        shape(&g),
        vec![(
            v(&["a", "b", "c"]),
            v(&["a", "c", "b", "a"]),
            Impact::Medium
        )]
    );
}

#[test]
fn an_edge_into_a_finished_component_does_not_merge_it_with_another() {
    let g = cycle_graph(&[("a", "b"), ("b", "a"), ("c", "a"), ("c", "d"), ("d", "c")]);
    let names: Vec<_> = shape(&g).into_iter().map(|c| c.0).collect();
    assert_eq!(names, vec![v(&["a", "b"]), v(&["c", "d"])]);
}
