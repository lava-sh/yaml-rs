use crate::load::types::NodeId;

#[derive(Clone, Debug)]
pub enum Value<'a> {
    Null,
    Boolean(bool),
    Integer64(i64),
    BigInteger(num_bigint::BigInt),
    Float(f64),
    String(&'a str),
    TaggedString(&'a str),
    Alias { target: NodeId, anchor_id: usize },
    Seq(Vec<NodeId>),
    Map(Vec<(NodeId, NodeId)>, bool),
}
