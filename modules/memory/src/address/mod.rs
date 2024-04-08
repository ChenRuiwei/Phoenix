mod offset;
mod phys;
mod step;
mod virt;

const PA_WIDTH_SV39: usize = 56;
const VA_WIDTH_SV39: usize = 39;
const PPN_WIDTH_SV39: usize = PA_WIDTH_SV39 - PAGE_SIZE_BITS;
const VPN_WIDTH_SV39: usize = VA_WIDTH_SV39 - PAGE_SIZE_BITS;

use config::mm::PAGE_SIZE_BITS;
pub use phys::{PhysAddr, PhysPageNum};
pub use step::StepByOne;
pub use virt::{VPNRange, VirtAddr, VirtPageNum};

macro_rules! impl_arithmetic_with_usize {
    ($t:ty) => {
        impl const core::ops::Add<usize> for $t {
            type Output = Self;
            #[inline]
            fn add(self, rhs: usize) -> Self {
                Self(self.0 + rhs)
            }
        }
        impl const core::ops::AddAssign<usize> for $t {
            #[inline]
            fn add_assign(&mut self, rhs: usize) {
                *self = *self + rhs;
            }
        }
        impl const core::ops::Sub<usize> for $t {
            type Output = Self;
            #[inline]
            fn sub(self, rhs: usize) -> Self {
                Self(self.0 - rhs)
            }
        }
        impl const core::ops::SubAssign<usize> for $t {
            #[inline]
            fn sub_assign(&mut self, rhs: usize) {
                *self = *self - rhs;
            }
        }
        impl const core::ops::Sub<$t> for $t {
            type Output = usize;
            #[inline]
            fn sub(self, rhs: $t) -> usize {
                self.0 - rhs.0
            }
        }
    };
}

macro_rules! impl_fmt {
    ($t:ty, $prefix:expr) => {
        impl fmt::Debug for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_fmt(format_args!("{}:{:#x}", $prefix, self.0))
            }
        }
        impl fmt::LowerHex for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_fmt(format_args!("{}:{:#x}", $prefix, self.0))
            }
        }
        impl fmt::UpperHex for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_fmt(format_args!("{}:{:#X}", $prefix, self.0))
            }
        }
    };
}

pub(self) use impl_arithmetic_with_usize;
pub(self) use impl_fmt;
