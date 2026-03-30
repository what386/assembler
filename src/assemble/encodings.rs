use crate::assemble::resolution::{ResolvedAltCondition, ResolvedCondition, ResolvedStdCondition};

pub fn encode_condition(condition: &ResolvedCondition) -> u8 {
    match condition {
        ResolvedCondition::Standard(condition) => encode_std_condition(condition),
        ResolvedCondition::Alternate(condition) => encode_alt_condition(condition),
    }
}

pub fn encode_std_condition(condition: &ResolvedStdCondition) -> u8 {
    match condition {
        ResolvedStdCondition::Equal => 0b000,
        ResolvedStdCondition::NotEqual => 0b001,
        ResolvedStdCondition::Lower => 0b010,
        ResolvedStdCondition::Higher => 0b011,
        ResolvedStdCondition::LowerSame => 0b100,
        ResolvedStdCondition::HigherSame => 0b101,
        ResolvedStdCondition::Even => 0b110,
        ResolvedStdCondition::Always => 0b111,
    }
}

pub fn encode_alt_condition(condition: &ResolvedAltCondition) -> u8 {
    match condition {
        ResolvedAltCondition::Overflow => 0b000,
        ResolvedAltCondition::NoOverflow => 0b001,
        ResolvedAltCondition::Less => 0b010,
        ResolvedAltCondition::Greater => 0b011,
        ResolvedAltCondition::LessEqual => 0b100,
        ResolvedAltCondition::GreaterEqual => 0b101,
        ResolvedAltCondition::Odd => 0b110,
        ResolvedAltCondition::Always => 0b111,
    }
}
