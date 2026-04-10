use anyhow::Result;

pub async fn run() -> Result<()> {
    let context = crazyflie_link::LinkContext::new();
    let found = context.scan([0xE7; 5]).await?;

    if found.is_empty() {
        eprintln!("No Crazyflies found");
        std::process::exit(1);
    }

    for uri in &found {
        println!("{}", uri);
    }

    Ok(())
}
