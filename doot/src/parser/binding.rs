#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum BindingPower {
    Default,
    Literal,
    Conditional,
    Additive,
    Multiplicative,
    Prefix,
    Postfix,
    Call,
}
