use super::{DefinitionVersion, DomainError, DomainResult, LocalRef, ModeId, ProfileId, require};

/// Versioned work objective; selecting resources is a separate policy concern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionMode {
    id: ModeId,
    version: DefinitionVersion,
    name: String,
    purpose: String,
}

impl ActionMode {
    pub fn new(
        id: ModeId,
        version: DefinitionVersion,
        name: impl Into<String>,
        purpose: impl Into<String>,
    ) -> DomainResult<Self> {
        let name = name.into();
        let purpose = purpose.into();
        require(
            !name.trim().is_empty() && !purpose.trim().is_empty(),
            DomainError::EmptyContent,
        )?;
        Ok(Self {
            id,
            version,
            name,
            purpose,
        })
    }

    pub fn id(&self) -> &ModeId {
        &self.id
    }
    pub fn version(&self) -> DefinitionVersion {
        self.version
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn purpose(&self) -> &str {
        &self.purpose
    }

    pub fn revise(
        &self,
        version: DefinitionVersion,
        name: impl Into<String>,
        purpose: impl Into<String>,
    ) -> DomainResult<Self> {
        require(version > self.version, DomainError::InvalidVersion)?;
        Self::new(self.id.clone(), version, name, purpose)
    }
}

/// Executor behavior, stored independently of mode and resource strategy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentProfile {
    id: ProfileId,
    version: DefinitionVersion,
    name: String,
    role: String,
    instructions: LocalRef,
}

impl AgentProfile {
    pub fn new(
        id: ProfileId,
        version: DefinitionVersion,
        name: impl Into<String>,
        role: impl Into<String>,
        instructions: LocalRef,
    ) -> DomainResult<Self> {
        let name = name.into();
        let role = role.into();
        require(
            !name.trim().is_empty() && !role.trim().is_empty(),
            DomainError::EmptyContent,
        )?;
        Ok(Self {
            id,
            version,
            name,
            role,
            instructions,
        })
    }

    pub fn id(&self) -> &ProfileId {
        &self.id
    }
    pub fn version(&self) -> DefinitionVersion {
        self.version
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn role(&self) -> &str {
        &self.role
    }
    pub fn instructions(&self) -> &LocalRef {
        &self.instructions
    }

    pub fn revise(
        &self,
        version: DefinitionVersion,
        name: impl Into<String>,
        role: impl Into<String>,
        instructions: LocalRef,
    ) -> DomainResult<Self> {
        require(version > self.version, DomainError::InvalidVersion)?;
        Self::new(self.id.clone(), version, name, role, instructions)
    }
}
