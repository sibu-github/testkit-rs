use std::{fmt::Display, str::FromStr};

use bson::Document;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CommandName {
    Find,
    FindAndModify,
    Insert,
    Update,
    Delete,
    Aggregate,
    Distinct,
    CreateIndex,
    Handshake,
    Unknown,
}

impl FromStr for CommandName {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "find" => Ok(Self::Find),
            "findAndModify" => Ok(Self::FindAndModify),
            "insert" => Ok(Self::Insert),
            "update" => Ok(Self::Update),
            "delete" => Ok(Self::Delete),
            "aggregate" => Ok(Self::Aggregate),
            "distinct" => Ok(Self::Distinct),
            "createIndexes" => Ok(Self::CreateIndex),
            "hello" | "isMaster" | "ismaster" | "ping" | "endSessions" => Ok(Self::Handshake),
            _ => Ok(Self::Unknown),
        }
    }
}

impl TryFrom<&Document> for CommandName {
    type Error = anyhow::Error;
    fn try_from(value: &Document) -> Result<Self, Self::Error> {
        let name = value
            .keys()
            .next()
            .map(String::to_string)
            .unwrap_or_default();
        let cmd = name.parse()?;
        Ok(cmd)
    }
}

impl Display for CommandName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Find => write!(f, "find"),
            Self::FindAndModify => write!(f, "findAndModify"),
            Self::Insert => write!(f, "insert"),
            Self::Update => write!(f, "update"),
            Self::Delete => write!(f, "delete"),
            Self::Aggregate => write!(f, "aggregate"),
            Self::Distinct => write!(f, "distinct"),
            Self::CreateIndex => write!(f, "createIndexes"),
            Self::Handshake => write!(f, "handshake"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}
