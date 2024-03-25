#![no_std]
#![no_main]
#![feature(negative_impls)]
#![feature(sync_unsafe_cell)]
#![feature(const_mut_refs)]

extern crate alloc;

#[macro_use]
extern crate bitflags;

pub mod cell;
pub mod mailbox;
pub mod mutex;

pub use mailbox::{Event, Mailbox};
