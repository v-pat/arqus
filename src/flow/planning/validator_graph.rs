use std::collections::{HashMap, HashSet, VecDeque};

use crate::flow::schema::types::TaskContract;

pub fn matches_pattern(pattern: &str, path: &str) -> bool {
    if let Some(root) = pattern.strip_suffix("/**") {
        let prefix = &pattern[..pattern.len() - 2];
        path.starts_with(prefix) || path == root
    } else {
        pattern == path
    }
}

pub fn build_transitive_deps(
    tasks: &[TaskContract],
) -> (HashMap<String, HashSet<String>>, Option<Vec<String>>) {
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_degrees: HashMap<String, usize> = HashMap::new();
    let mut direct_deps: HashMap<String, Vec<String>> = HashMap::new();

    for t in tasks {
        in_degrees.insert(t.id.clone(), 0);
        adj.insert(t.id.clone(), Vec::new());
        direct_deps.insert(t.id.clone(), t.depends_on.clone());
    }

    for t in tasks {
        for dep in &t.depends_on {
            if in_degrees.contains_key(dep) {
                adj.entry(dep.clone()).or_default().push(t.id.clone());
                *in_degrees.entry(t.id.clone()).or_default() += 1;
            }
        }
    }

    let mut q: VecDeque<String> = VecDeque::new();
    for (id, deg) in &in_degrees {
        if *deg == 0 {
            q.push_back(id.clone());
        }
    }

    let mut visited_count = 0;
    while let Some(u) = q.pop_front() {
        visited_count += 1;
        if let Some(neighbors) = adj.get(&u) {
            for v in neighbors {
                if let Some(deg) = in_degrees.get_mut(v) {
                    *deg -= 1;
                    if *deg == 0 {
                        q.push_back(v.clone());
                    }
                }
            }
        }
    }

    let cycle = if visited_count < tasks.len() {
        let mut cycle_ids: Vec<String> = in_degrees
            .into_iter()
            .filter(|(_, deg)| *deg > 0)
            .map(|(id, _)| id)
            .collect();
        cycle_ids.sort();
        Some(cycle_ids)
    } else {
        None
    };

    let mut trans: HashMap<String, HashSet<String>> = HashMap::new();
    for t in tasks {
        let mut all_deps = HashSet::new();
        let mut stack = t.depends_on.clone();
        while let Some(cur) = stack.pop() {
            if all_deps.insert(cur.clone()) {
                if let Some(nexts) = direct_deps.get(&cur) {
                    for n in nexts {
                        if !all_deps.contains(n) {
                            stack.push(n.clone());
                        }
                    }
                }
            }
        }
        trans.insert(t.id.clone(), all_deps);
    }

    (trans, cycle)
}

#[cfg(test)]
#[path = "validator_graph_tests.rs"]
mod tests;
