//! L2 kernel registry and MVP0 FSST stub.
//!
//! L2 kernels are total functions that own their output Arrow array. This keeps
//! the L1 read loop focused on builder-backed declarative encodings while
//! preserving a clean seam for Phase 5's real FSST implementation.

use arrow::array::{Array, StringArray};
use arrow_data::ArrayData;

use crate::error::LoomDecodeError;

/// Total-function L2 kernel.
pub trait L2Kernel {
    /// Decode kernel-specific params into an Arrow array.
    fn decode(&self, params: &[u8], count: usize) -> Result<ArrayData, LoomDecodeError>;
}

/// Registry mapping stable kernel ids to L2 kernels.
pub struct L2KernelRegistry {
    kernels: Vec<Box<dyn L2Kernel>>,
}

impl L2KernelRegistry {
    /// Construct the MVP0 registry. Kernel id 0 is the FSST stub.
    pub fn default_for_mvp0() -> Self {
        Self {
            kernels: vec![Box::new(FsstKernel)],
        }
    }

    /// Return the registered kernel for `id`, if any.
    pub fn get(&self, id: u32) -> Option<&dyn L2Kernel> {
        self.kernels.get(id as usize).map(|k| k.as_ref())
    }
}

/// Phase-4 FSST placeholder.
///
/// The real Phase-5 body will decode FSST strings. The Phase-4 contract is
/// type-accurate routing: return an empty Utf8 array owned by the kernel.
pub struct FsstKernel;

impl L2Kernel for FsstKernel {
    fn decode(&self, _params: &[u8], _count: usize) -> Result<ArrayData, LoomDecodeError> {
        let array = StringArray::from(Vec::<&str>::new());
        Ok(array.into_data())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_schema::DataType;

    #[test]
    fn default_registry_has_fsst_at_zero() {
        let registry = L2KernelRegistry::default_for_mvp0();
        let kernel = registry
            .get(0)
            .expect("FSST stub must be registered at id 0");
        let data = kernel.decode(&[], 0).expect("stub decode should succeed");
        assert_eq!(data.data_type(), &DataType::Utf8);
        assert_eq!(data.len(), 0);
    }

    #[test]
    fn default_registry_missing_id_returns_none() {
        let registry = L2KernelRegistry::default_for_mvp0();
        assert!(registry.get(1).is_none());
    }
}
