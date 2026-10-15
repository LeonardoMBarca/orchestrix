use crate::{ProcessError, Result, SpawnSpec, TreeState};
pub(crate) struct NativeProcess;
pub(crate) struct Launched {
    pub process: NativeProcess,
    pub input: std::fs::File,
    pub output: std::fs::File,
    pub error: std::fs::File,
}
impl NativeProcess {
    pub fn member_ids(&self) -> Result<Vec<u32>> {
        Err(ProcessError::Unsupported)
    }
    pub fn spawn(_: &SpawnSpec, _: Option<&crate::named_job::JobScope>) -> Result<Launched> {
        Err(ProcessError::Unsupported)
    }
    pub fn observe(&self) -> Result<TreeState> {
        Err(ProcessError::Unsupported)
    }
    pub fn terminate(&self) -> Result<()> {
        Err(ProcessError::Unsupported)
    }
}
