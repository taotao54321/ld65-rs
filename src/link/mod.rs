use crate::object::Object;
use crate::script::LinkScript;

mod emit;
mod graph;
mod layout;
mod symbol;

use self::graph::LinkGraph;
use self::layout::LinkLayout;
use self::symbol::SymbolTable;

pub fn link(script: &LinkScript, objs: &[Object]) -> LinkOutputs {
    let graph = LinkGraph::new(script, objs);

    let layout = LinkLayout::new(script, objs, &graph);

    let sym_table = SymbolTable::new(objs, &graph, &layout);

    let mut outputs = Vec::<LinkOutput>::with_capacity(graph.file_count());

    for file_i in graph.files() {
        let body = self::emit::emit_file(objs, &graph, &layout, &sym_table, file_i);
        let output = LinkOutput {
            path: graph.file_name(file_i).to_owned(),
            body,
        };
        outputs.push(output);
    }

    LinkOutputs {
        outputs: outputs.into(),
    }
}

#[derive(Debug)]
pub struct LinkOutputs {
    outputs: Box<[LinkOutput]>,
}

impl LinkOutputs {
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = &LinkOutput> + std::iter::FusedIterator + Clone {
        self.outputs.iter()
    }
}

#[derive(Debug)]
pub struct LinkOutput {
    path: String,
    body: Box<[u8]>,
}

impl LinkOutput {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}
