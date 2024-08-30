use aptos_types::{state_store::StateView, transaction::{SignedTransaction, VMValidatorResult}};

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

pub struct AptosVM;


pub trait VMValidator {
    /// Executes the prologue of the Aptos Account and verifies that the transaction is valid.
    fn validate_transaction(
        &self,
        transaction: SignedTransaction,
        state_view: &impl StateView,
    ) -> VMValidatorResult;
}

impl VMValidator for AptosVM {
    /// Executes the prologue of the Aptos Account and verifies that the transaction is valid.
    fn validate_transaction(
        &self,
        transaction: SignedTransaction,
        state_view: &impl StateView,
    ) -> VMValidatorResult {
        todo!()
    }
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
