use crate::load::value::Value;

pub(crate) type NodeId = usize;

#[derive(Debug)]
pub(crate) struct Arena<'a> {
    nodes: Vec<Value<'a>>,
}

impl<'a> Arena<'a> {
    #[inline]
    pub(crate) fn with_capacity(cap: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(cap),
        }
    }

    #[inline]
    pub(crate) fn push(&mut self, value: Value<'a>) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(value);
        id
    }

    #[inline]
    pub(crate) fn get(&self, id: NodeId) -> &Value<'_> {
        // SAFETY: `id` is produced by `push` and nodes are never removed,
        // so it is always a valid index.
        unsafe { self.nodes.get_unchecked(id) }
    }
}
