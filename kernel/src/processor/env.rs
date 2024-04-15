use super::local_env_mut;

/// use RAII to guard `sum` flag
pub struct SumGuard;

impl SumGuard {
    pub fn new() -> Self {
        local_env_mut().sum_inc();
        Self {}
    }
}

impl Drop for SumGuard {
    fn drop(&mut self) {
        local_env_mut().sum_dec();
    }
}
