use clap::Parser;
use subroute::app::{Cli, run};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match run(cli).await {
        Ok(path) => {
            println!("报告已生成: {}", path.display());
        }
        Err(err) => {
            eprintln!("执行失败: {err:#}");
            std::process::exit(1);
        }
    }
}
