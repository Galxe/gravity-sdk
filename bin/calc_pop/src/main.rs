use clap::Parser;
use gaptos::aptos_crypto::{bls12381::ProofOfPossession, PrivateKey, ValidCryptoMaterial};
use std::convert::TryFrom;

#[derive(Debug, Parser)]
#[command(name = "calc_pop", about = "Calculate Consensus Proof of Possession from a BLS private key")]
struct CalcPop {
    /// The BLS12-381 private key hex string
    #[clap(long)]
    private_key: String,
}

fn main() -> Result<(), anyhow::Error> {
    let args = CalcPop::parse();
    
    let pk_hex = args.private_key.strip_prefix("0x").unwrap_or(&args.private_key);
    let bytes = hex::decode(pk_hex)
        .map_err(|e| anyhow::anyhow!("Failed to decode hex string: {}", e))?;
    
    let private_key = gaptos::aptos_crypto::bls12381::PrivateKey::try_from(bytes.as_slice())
        .map_err(|e| anyhow::anyhow!("Invalid private key: {:?}", e))?;
    
    let pop = ProofOfPossession::create(&private_key);
    let pop_hex = hex::encode(pop.to_bytes());
    
    println!("consensus_pop: {pop_hex}");
    Ok(())
}
