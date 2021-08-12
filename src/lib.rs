pub mod sha256;
mod table16;

pub use table16::*;
pub use sha256::*;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
