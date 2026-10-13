use super::{DomainError, DomainResult};

macro_rules! local_identity {
    ($($name:ident),+ $(,)?) => { $(
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);
        impl $name {
            pub fn new(value: impl Into<String>) -> DomainResult<Self> {
                let value = value.into();
                if value.is_empty() || value.len() > 80 || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
                    return Err(DomainError::InvalidReference);
                }
                Ok(Self(value))
            }
            pub fn as_str(&self) -> &str { &self.0 }
        }
    )+ };
}
local_identity!(
    ProjectId,
    RepositoryId,
    ConnectionId,
    CapacityGroupId,
    RunId,
    TaskId,
    AttemptId,
    ConversationId,
    RuntimeSessionId,
    ModeId,
    ProfileId,
    ContextPackId,
    ArtifactId,
    RevisionId,
    EvidenceId,
    LocalRef
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DefinitionVersion(u64);
impl DefinitionVersion {
    pub fn new(value: u64) -> DomainResult<Self> {
        if value == 0 {
            Err(DomainError::InvalidVersion)
        } else {
            Ok(Self(value))
        }
    }
    pub fn get(self) -> u64 {
        self.0
    }
}
