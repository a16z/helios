use std::fmt::Display;

use ssz_rs::Vector;

pub type Bytes32 = Vector<u8, 32>;

#[derive(Debug)]
pub enum BlockTag {
    Latest,
    Finalized,
    Number(u64),
}

impl Display for BlockTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let formatted = match self {
            Self::Latest => "latest".to_string(),
            Self::Finalized => "finalized".to_string(),
            Self::Number(num) => num.to_string(),
        };

        write!(f, "{}", formatted)
    }
}
