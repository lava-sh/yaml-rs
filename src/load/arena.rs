use crate::load::value::Value;

pub type NodeId = usize;

#[derive(Debug)]
pub struct Arena {
    nodes: Vec<Value>,
}

impl Arena {
    #[inline]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(cap),
        }
    }

    #[inline]
    pub fn push(&mut self, value: Value) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(value);
        id
    }

    #[inline]
    pub fn get(&self, id: NodeId) -> &Value {
        unsafe { self.nodes.get_unchecked(id) }
    }
}
