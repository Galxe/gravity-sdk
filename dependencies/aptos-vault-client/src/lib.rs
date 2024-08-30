pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

pub struct Client {
    
}

pub mod dev {

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
