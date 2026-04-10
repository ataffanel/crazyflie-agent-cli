use anyhow::{Result, bail};
use crazyflie_link::LinkContext;

use crate::output::{print_tagged, Tag};

pub async fn run(uri: Option<&str>) -> Result<()> {
    let context = LinkContext::new();

    let cf_uri = if let Some(u) = uri {
        u.to_string()
    } else {
        let found = context.scan([0xE7; 5]).await?;
        if found.is_empty() {
            bail!("No Crazyflie found on radio");
        }
        found[0].clone()
    };

    let link = context.open_link(&cf_uri).await?;

    // Send reset-init then reset-to-firmware (same as cfcli reboot)
    let packet: crazyflie_link::Packet = vec![0xFF, 0xFE, 0xFF].into();
    link.send_packet(packet).await?;

    let packet: crazyflie_link::Packet = vec![0xFF, 0xFE, 0xF0, 0x01].into();
    link.send_packet(packet).await?;

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    print_tagged(Tag::Status, "Crazyflie rebooted");

    Ok(())
}
