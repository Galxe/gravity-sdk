use crate::reth_cli::RethCli;

pub struct ExecLayer {
    pub reth_cli: RethCli,
}

impl ExecLayer {
    pub fn new(reth_cli: RethCli) -> Self {
        Self { reth_cli }
    }

   pub async fn  run(&self) {
       println!("run ExecLayer");
        self.reth_cli.process_pending_transactions(|| ()).await.expect("\
            Error processing pending transactions");
    }
}