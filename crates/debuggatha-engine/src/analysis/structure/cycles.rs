//! Cyclic dependencies: the strongly connected components of the dependency graph (Tarjan). One
//! cycle per component, not per elementary cycle inside it, so a tangle of overlapping loops is
//! one finding.

use std::collections::{BTreeMap, VecDeque};

use serde::Serialize;

use super::graph::DependencyGraph;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Impact {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DependencyCycle {
    /// The modules in the cycle, sorted.
    pub modules: Vec<String>,
    /// The shortest path that leaves the first module and comes back to it.
    pub shortest_cycle: Vec<String>,
    pub impact: Impact,
}

struct Tarjan<'g> {
    adjacency: &'g BTreeMap<&'g str, Vec<&'g str>>,
    index: usize,
    indices: BTreeMap<&'g str, usize>,
    lowlink: BTreeMap<&'g str, usize>,
    on_stack: Vec<&'g str>,
    components: Vec<Vec<&'g str>>,
}

impl<'g> Tarjan<'g> {
    /// Iterative, so a graph of thousands of modules cannot overflow the stack.
    fn run(&mut self, root: &'g str) {
        // (node, next neighbour to look at)
        let mut work: Vec<(&'g str, usize)> = vec![(root, 0)];
        self.visit(root);
        while let Some((node, next)) = work.pop() {
            let neighbours = &self.adjacency[node];
            if let Some(&w) = neighbours.get(next) {
                work.push((node, next + 1));
                if !self.indices.contains_key(w) {
                    self.visit(w);
                    work.push((w, 0));
                } else if self.on_stack.contains(&w) {
                    let low = self.lowlink[node].min(self.indices[w]);
                    self.lowlink.insert(node, low);
                }
                continue;
            }
            if self.lowlink[node] == self.indices[node] {
                let mut component = Vec::new();
                while let Some(w) = self.on_stack.pop() {
                    component.push(w);
                    if w == node {
                        break;
                    }
                }
                self.components.push(component);
            }
            if let Some(&(parent, _)) = work.last() {
                let low = self.lowlink[parent].min(self.lowlink[node]);
                self.lowlink.insert(parent, low);
            }
        }
    }

    fn visit(&mut self, node: &'g str) {
        self.indices.insert(node, self.index);
        self.lowlink.insert(node, self.index);
        self.index += 1;
        self.on_stack.push(node);
    }
}

/// Every cycle of the graph, sorted by its modules.
#[must_use]
pub fn detect_cycles(graph: &DependencyGraph) -> Vec<DependencyCycle> {
    let mut adjacency: BTreeMap<&str, Vec<&str>> = graph
        .nodes
        .iter()
        .map(|n| (n.as_str(), Vec::new()))
        .collect();
    for edge in &graph.edges {
        if let Some(list) = adjacency.get_mut(edge.from.as_str()) {
            list.push(edge.to.as_str());
        }
    }
    let mut tarjan = Tarjan {
        adjacency: &adjacency,
        index: 0,
        indices: BTreeMap::new(),
        lowlink: BTreeMap::new(),
        on_stack: Vec::new(),
        components: Vec::new(),
    };
    for node in &graph.nodes {
        if !tarjan.indices.contains_key(node.as_str()) {
            tarjan.run(node);
        }
    }
    let mut cycles: Vec<DependencyCycle> = tarjan
        .components
        .into_iter()
        .filter(|c| c.len() >= 2)
        .map(|component| {
            let mut modules: Vec<String> = component.iter().map(|m| (*m).to_owned()).collect();
            modules.sort_by(|a, b| debuggatha_core::js::utf16_cmp(a, b));
            let shortest_cycle = shortest_cycle(&modules, &adjacency);
            let impact = match modules.len() {
                0..=2 => Impact::Low,
                3 => Impact::Medium,
                _ => Impact::High,
            };
            DependencyCycle {
                modules,
                shortest_cycle,
                impact,
            }
        })
        .collect();
    cycles
        .sort_by(|a, b| debuggatha_core::js::utf16_cmp(&a.modules.join(","), &b.modules.join(",")));
    cycles
}

/// Breadth-first from the first module back to itself, inside the component.
fn shortest_cycle(component: &[String], adjacency: &BTreeMap<&str, Vec<&str>>) -> Vec<String> {
    let Some(start) = component.first() else {
        return Vec::new();
    };
    let inside = |m: &str| component.iter().any(|c| c == m);
    let mut queue: VecDeque<Vec<&str>> = VecDeque::from([vec![start.as_str()]]);
    let mut visited = vec![start.as_str()];
    while let Some(path) = queue.pop_front() {
        let Some(&last) = path.last() else {
            continue;
        };
        for &next in adjacency.get(last).map_or(&[][..], Vec::as_slice) {
            // Both checks bound the search and cannot change the answer: a way back to `start`
            // through a module outside the component would put that module in the component, and
            // a path that revisits a module is never the shortest.
            if !inside(next) {
                continue;
            }
            if next == start {
                let mut done: Vec<String> = path.iter().map(|m| (*m).to_owned()).collect();
                done.push(start.clone());
                return done;
            }
            if visited.contains(&next) {
                continue;
            }
            visited.push(next);
            let mut longer = path.clone();
            longer.push(next);
            queue.push_back(longer);
        }
    }
    component.to_vec()
}
