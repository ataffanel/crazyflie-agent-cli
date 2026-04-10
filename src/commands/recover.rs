use anyhow::Result;
use crazyflie_link::LinkContext;

use crate::output::{print_tagged, Tag};

pub async fn run() -> Result<()> {
    let context = LinkContext::new();

    // Step 1: Try to reach the Crazyflie normally
    print_tagged(Tag::Recover, "scanning for Crazyflie...");
    let found = context.scan([0xE7; 5]).await?;

    if !found.is_empty() {
        // Crazyflie is reachable - try to reset it to bootloader
        print_tagged(Tag::Recover, &format!("found {} - attempting reset to bootloader", found[0]));
        let link = context.open_link(&found[0]).await?;
        let packet: crazyflie_link::Packet = vec![0xFF, 0xFE, 0xFF].into();
        link.send_packet(packet).await?;
        let packet: crazyflie_link::Packet = vec![0xFF, 0xFE, 0xF0, 0x00].into();
        link.send_packet(packet).await?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        link.close().await;
        print_tagged(Tag::Recover, "reset command sent, Crazyflie should be in bootloader mode");
        return Ok(());
    }

    // Step 2: Can't reach it - ask human for help
    print_tagged(Tag::Recover, "cannot reach Crazyflie via radio");
    print_tagged(Tag::Recover, "ACTION REQUIRED: please restart the Crazyflie in bootloader mode");
    print_tagged(Tag::Recover, "  1. Turn off the Crazyflie");
    print_tagged(Tag::Recover, "  2. Hold the power button for 3 seconds until blue LEDs blink");
    print_tagged(Tag::Recover, "waiting for Crazyflie in bootloader mode...");

    // Step 3: Poll for bootloader
    loop {
        let found = context.scan_selected(
            vec!["radio://0/110/2M/E7E7E7E7E7", "radio://0/0/2M/E7E7E7E7E7"]
        ).await?;

        if !found.is_empty() {
            print_tagged(Tag::Recover, &format!("found Crazyflie in bootloader mode at {}", found[0]));
            print_tagged(Tag::Recover, "ready to flash");
            return Ok(());
        }

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
