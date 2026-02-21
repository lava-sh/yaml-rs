use std::borrow::Cow;

use crate::load::arena::NodeId;

#[derive(Clone, Debug)]
pub(crate) enum Value<'a> {
    Null,
    Boolean(bool),
    IntegerI64(i64),
    IntegerBig(num_bigint::BigInt),
    Float(f64),
    String(Cow<'a, str>),
    StringExplicit(Cow<'a, str>),
    Seq(Vec<NodeId>),
    Map(Vec<(NodeId, NodeId)>),
}
