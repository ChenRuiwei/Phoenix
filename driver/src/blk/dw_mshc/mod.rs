mod dma;
mod mmc;
mod registers;

use alloc::sync::Arc;

use fdt::Fdt;
use log::{info, warn};
pub use mmc::MMC;


