#![no_std]
#![no_main]
pub fn add(left: usize, right: usize) -> usize {
    left + right
}
pub mod action;
pub mod signal_stack;
pub mod sigset;

pub use action::Signal;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
