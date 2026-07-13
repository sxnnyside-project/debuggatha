import type { DependencyCycle, DependencyGraph } from "../types.js";

/**
 * Cyclic Dependencies (Epic 12): Tarjan's strongly-connected-components
 * over the Dependency Graph — every SCC with more than one node is a
 * cycle (a single self-referencing node never occurs; self-edges are
 * excluded when the graph is built). One `DependencyCycle` per SCC, not
 * per elementary cycle within it — "avoid duplicated findings for the
 * same cycle" when a component has multiple internal cycles sharing
 * nodes.
 */
export function detectCycles(graph: DependencyGraph): DependencyCycle[] {
  const adjacency = new Map<string, string[]>();
  for (const node of graph.nodes) adjacency.set(node, []);
  for (const edge of graph.edges) {
    adjacency.get(edge.from)?.push(edge.to);
  }

  let index = 0;
  const indices = new Map<string, number>();
  const lowlink = new Map<string, number>();
  const onStack = new Set<string>();
  const stack: string[] = [];
  const sccs: string[][] = [];

  function strongConnect(v: string) {
    indices.set(v, index);
    lowlink.set(v, index);
    index += 1;
    stack.push(v);
    onStack.add(v);

    for (const w of adjacency.get(v) ?? []) {
      if (!indices.has(w)) {
        strongConnect(w);
        lowlink.set(v, Math.min(lowlink.get(v) as number, lowlink.get(w) as number));
      } else if (onStack.has(w)) {
        lowlink.set(v, Math.min(lowlink.get(v) as number, indices.get(w) as number));
      }
    }

    if (lowlink.get(v) === indices.get(v)) {
      const component: string[] = [];
      let w: string | undefined;
      do {
        w = stack.pop();
        if (w === undefined) break;
        onStack.delete(w);
        component.push(w);
      } while (w !== v);
      sccs.push(component);
    }
  }

  for (const node of graph.nodes) {
    if (!indices.has(node)) strongConnect(node);
  }

  const cycles: DependencyCycle[] = [];
  for (const component of sccs) {
    if (component.length < 2) continue;
    const modules = [...component].sort();
    const shortestCycle = findShortestCycle(modules, adjacency);
    cycles.push({
      modules,
      shortestCycle,
      impact: modules.length >= 4 ? "high" : modules.length === 3 ? "medium" : "low",
    });
  }

  return cycles.sort((a, b) => a.modules.join(",").localeCompare(b.modules.join(",")));
}

/** BFS from the lexicographically-first module in the component back to itself, restricted to the component's own nodes. */
function findShortestCycle(component: string[], adjacency: Map<string, string[]>): string[] {
  const start = component[0];
  if (start === undefined) return [];
  const inComponent = new Set(component);

  const queue: string[][] = [[start]];
  const visited = new Set<string>([start]);

  while (queue.length > 0) {
    const path = queue.shift();
    if (!path) continue;
    const last = path[path.length - 1];
    if (last === undefined) continue;
    for (const next of adjacency.get(last) ?? []) {
      if (!inComponent.has(next)) continue;
      if (next === start) return [...path, start];
      if (visited.has(next)) continue;
      visited.add(next);
      queue.push([...path, next]);
    }
  }

  return component;
}
