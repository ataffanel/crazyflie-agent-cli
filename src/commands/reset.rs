use anyhow::{Result, bail};
use crazyflie_link::LinkContext;

use crate::output::{print_tagged, Tag};

pub async fn run() -> Result<()> {
    let context = LinkContext::new();

    let found = context.scan([0xE7; 5]).await?;
    if found.is_empty() {
        bail!("No Crazyflie found on radio");
    }

    let uri = &found[0];
    let link = context.open_link(uri).await?;

    // Send reset-init then reset-to-firmware (same as cfcli reboot)
    let packet: crazyflie_link::Packet = vec![0xFF, 0xFE, 0xFF].into();
    link.send_packet(packet).await?;

    let packet: crazyflie_link::Packet = vec![0xFF, 0xFE, 0xF0, 0x01].into();
    link.send_packet(packet).await?;

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    print_tagged(Tag::Status, "Crazyflie rebooted");

    Ok(())
}
