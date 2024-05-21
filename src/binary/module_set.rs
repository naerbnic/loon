use std::collections::HashMap;

use super::{modules::ModuleId, ConstModule};

fn detect_cycles<T>(edges: HashMap<T, Vec<T>>) -> bool
where
    T: Eq + std::hash::Hash,
{
    let mut visited = HashMap::new();

    fn visit<'a, T>(
        node: &'a T,
        edges: &'a HashMap<T, Vec<T>>,
        visited: &mut HashMap<&'a T, bool>,
    ) -> bool
    where
        T: Eq + std::hash::Hash,
    {
        match visited.get(&node) {
            Some(&true) => true,
            Some(&false) => false,
            None => {
                visited.insert(node, true);
                let result = edges
                    .get(node)
                    .map(|node_edges| node_edges.iter().any(|edge| visit(edge, edges, visited)))
                    .unwrap_or(false);
                visited.insert(node, false);
                result
            }
        }
    }

    edges.keys().any(|node| visit(node, &edges, &mut visited))
}

pub struct ModuleSet {
    modules: HashMap<ModuleId, ConstModule>,
}

impl ModuleSet {
    pub fn new(modules: impl IntoIterator<Item = ConstModule>) -> Self {
        let modules: HashMap<ModuleId, ConstModule> = modules
            .into_iter()
            .map(|module| (module.id().clone(), module))
            .collect();

        let dependency_edges = modules
            .iter()
            .map(|(id, module)| (id, module.dependencies().collect()))
            .collect();

        if detect_cycles(dependency_edges) {
            panic!("Cyclic module dependencies detected.");
        }

        Self { modules }
    }

    pub fn external_dependencies(&self) -> impl Iterator<Item = &ModuleId> {
        self.modules
            .values()
            .flat_map(|module| module.dependencies())
            .filter(|id| !self.modules.contains_key(id))
    }

    pub fn modules(&self) -> impl Iterator<Item = &ConstModule> {
        self.modules.values()
    }
}
