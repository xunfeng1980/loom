//! SMT-ready local proof-obligation IR for `L2Core`.
//!
//! The IR is solver-neutral. Phase 13 starts with deterministic diagnostic text
//! rather than a Z3 or SMT-LIB dependency.

use std::fmt;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub enum IntegerType {
    Int32,
    Int64,
    UInt32,
    UInt64,
    RowIndex,
}

impl fmt::Display for IntegerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            IntegerType::Int32 => "Int32",
            IntegerType::Int64 => "Int64",
            IntegerType::UInt32 => "UInt32",
            IntegerType::UInt64 => "UInt64",
            IntegerType::RowIndex => "RowIndex",
        };
        f.write_str(name)
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub enum ConstraintTerm {
    Var(String),
    Int(i128),
    Add(Box<ConstraintTerm>, Box<ConstraintTerm>),
    Sub(Box<ConstraintTerm>, Box<ConstraintTerm>),
    Mul(Box<ConstraintTerm>, Box<ConstraintTerm>),
}

impl ConstraintTerm {
    pub fn var(name: impl Into<String>) -> Self {
        Self::Var(name.into())
    }

    pub fn int(value: i128) -> Self {
        Self::Int(value)
    }
}

impl fmt::Display for ConstraintTerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstraintTerm::Var(name) => f.write_str(name),
            ConstraintTerm::Int(value) => write!(f, "{value}"),
            ConstraintTerm::Add(lhs, rhs) => write!(f, "(+ {lhs} {rhs})"),
            ConstraintTerm::Sub(lhs, rhs) => write!(f, "(- {lhs} {rhs})"),
            ConstraintTerm::Mul(lhs, rhs) => write!(f, "(* {lhs} {rhs})"),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub enum LoomConstraint {
    Le {
        id: String,
        left: ConstraintTerm,
        right: ConstraintTerm,
    },
    Lt {
        id: String,
        left: ConstraintTerm,
        right: ConstraintTerm,
    },
    Eq {
        id: String,
        left: ConstraintTerm,
        right: ConstraintTerm,
    },
    AddNoOverflow {
        id: String,
        left: ConstraintTerm,
        right: ConstraintTerm,
        ty: IntegerType,
    },
    MulNoOverflow {
        id: String,
        left: ConstraintTerm,
        right: ConstraintTerm,
        ty: IntegerType,
    },
    InRange {
        id: String,
        value: ConstraintTerm,
        lower: ConstraintTerm,
        upper_exclusive: ConstraintTerm,
    },
    Decreases {
        id: String,
        previous: ConstraintTerm,
        next: ConstraintTerm,
    },
    NonNegative {
        id: String,
        value: ConstraintTerm,
    },
    FeatureImplies {
        id: String,
        feature: String,
        obligation_id: String,
    },
}

impl LoomConstraint {
    pub fn id(&self) -> &str {
        match self {
            LoomConstraint::Le { id, .. }
            | LoomConstraint::Lt { id, .. }
            | LoomConstraint::Eq { id, .. }
            | LoomConstraint::AddNoOverflow { id, .. }
            | LoomConstraint::MulNoOverflow { id, .. }
            | LoomConstraint::InRange { id, .. }
            | LoomConstraint::Decreases { id, .. }
            | LoomConstraint::NonNegative { id, .. }
            | LoomConstraint::FeatureImplies { id, .. } => id,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            LoomConstraint::Le { .. } => "Le",
            LoomConstraint::Lt { .. } => "Lt",
            LoomConstraint::Eq { .. } => "Eq",
            LoomConstraint::AddNoOverflow { .. } => "AddNoOverflow",
            LoomConstraint::MulNoOverflow { .. } => "MulNoOverflow",
            LoomConstraint::InRange { .. } => "InRange",
            LoomConstraint::Decreases { .. } => "Decreases",
            LoomConstraint::NonNegative { .. } => "NonNegative",
            LoomConstraint::FeatureImplies { .. } => "FeatureImplies",
        }
    }

    fn detail(&self) -> String {
        match self {
            LoomConstraint::Le { left, right, .. } => format!("{left} <= {right}"),
            LoomConstraint::Lt { left, right, .. } => format!("{left} < {right}"),
            LoomConstraint::Eq { left, right, .. } => format!("{left} = {right}"),
            LoomConstraint::AddNoOverflow {
                left, right, ty, ..
            } => {
                format!("checked-add {left} {right} : {ty}")
            }
            LoomConstraint::MulNoOverflow {
                left, right, ty, ..
            } => {
                format!("checked-mul {left} {right} : {ty}")
            }
            LoomConstraint::InRange {
                value,
                lower,
                upper_exclusive,
                ..
            } => {
                format!("{lower} <= {value} < {upper_exclusive}")
            }
            LoomConstraint::Decreases { previous, next, .. } => {
                format!("{next} < {previous}")
            }
            LoomConstraint::NonNegative { value, .. } => format!("0 <= {value}"),
            LoomConstraint::FeatureImplies {
                feature,
                obligation_id,
                ..
            } => {
                format!("feature {feature} implies {obligation_id}")
            }
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default, PartialEq, Eq)]
pub struct ConstraintSet {
    constraints: Vec<LoomConstraint>,
}

impl ConstraintSet {
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
        }
    }

    pub fn push(&mut self, constraint: LoomConstraint) {
        self.constraints.push(constraint);
    }

    pub fn iter(&self) -> impl Iterator<Item = &LoomConstraint> {
        self.constraints.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    pub fn to_smtlib_comments(&self) -> String {
        let mut out = String::new();
        for constraint in &self.constraints {
            out.push_str("; loom-constraint ");
            out.push_str(constraint.id());
            out.push(' ');
            out.push_str(constraint.kind());
            out.push_str(": ");
            out.push_str(&constraint.detail());
            out.push('\n');
        }
        out
    }
}
