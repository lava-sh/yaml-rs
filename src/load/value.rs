use crate::load::arena::NodeId;

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Boolean(bool),
    IntegerI64(i64),
    IntegerBig(num_bigint::BigInt),
    Float(f64),
    String(String),
    StringExplicit(String),
    Seq(Vec<NodeId>),
    Map(Vec<(NodeId, NodeId)>),
}
