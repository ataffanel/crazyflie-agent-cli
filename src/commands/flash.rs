use anyhow::{anyhow, Result};
use crazyflie_link::{LinkContext, Packet};
use tokio::time::{sleep, Duration};

use crate::daemon::find_socket_path;
use crate::output::{print_tagged, Tag};
use cfloader::Bllink;

const TARGET_NRF51: u8 = 0xFE;

/// Warm-boot a running Crazyflie into bootloader mode and return the 5-byte
/// bootloader address.
async fn warm_boot_to_bootloader(uri: &str) -> Result<[u8; 5]> {
    let context = LinkContext::new();
    let separator = if uri.contains('?') { "&" } else { "?" };
    let link = context
        .open_link(&format!("{}{}safelink=0", uri, separator))
        .await?;

    // Disable safelink on the nRF51 side so it forwards bootloader messages
    let packet: Packet = vec![0xFF, TARGET_NRF51, 0xFF, 0x05, 0x00].into();
    link.send_packet(packet).await?;

    // Send reset-to-bootloader init command
    let packet: Packet = vec![0xFF, TARGET_NRF51, 0xFF].into();
    link.send_packet(packet).await?;

    // Wait for the new bootloader address in the response
    let mut new_address: Vec<u8> = Vec::new();
    loop {
        let packet = tokio::select! {
            result = link.recv_packet() => result?,
            _ = sleep(Duration::from_millis(100)) => {
                return Err(anyhow!("Timeout waiting for bootloader address"));
            }
        };
        let data = packet.get_data();
        if data.len() > 2 && data[0..2] == [TARGET_NRF51, 0xFF] {
            new_address.push(0xb1);
            for byte in data[2..6].iter().rev() {
                new_address.push(*byte);
            }
            break;
        }
    }

    // Confirm the reset
    for _ in 0..10 {
        let packet: Packet = vec![0xFF, TARGET_NRF51, 0xF0, 0x00].into();
        link.send_packet(packet).await?;
    }
    sleep(Duration::from_millis(500)).await;
    link.close().await;

    let address: [u8; 5] = new_address
        .try_into()
        .map_err(|_| anyhow!("Bootloader address must be exactly 5 bytes"))?;
    Ok(address)
}

/// Stop a running daemon session by sending it a Stop request.
async fn stop_daemon(sock_path: &std::path::Path) -> Result<()> {
    use tokio::io::{AsyncWriteExt, BufReader, AsyncBufReadExt};
    use tokio::net::UnixStream;
    use crate::protocol::Request;

    let stream = UnixStream::connect(sock_path).await?;
    let (reader, mut writer) = stream.into_split();

    let json = serde_json::to_string(&Request::Stop)?;
    writer.write_all(json.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    // Read (and discard) the response so the daemon has time to handle it
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let _ = reader.read_line(&mut line).await;

    Ok(())
}

pub async fn run(path: &str) -> Result<()> {
    // 1. Read the firmware binary
    let data = std::fs::read(path)?;
    print_tagged(Tag::Flash, &format!("loaded {} bytes from {}", data.len(), path));

    // 2. If a daemon session is running, stop it to free the radio
    if let Some(sock_path) = find_socket_path() {
        print_tagged(Tag::Flash, "stopping daemon to free radio access");
        stop_daemon(&sock_path).await?;
        sleep(Duration::from_secs(1)).await;
    }

    // 3. Try to connect directly in bootloader mode first (Crazyflie may already be in bootloader)
    let bllink_result = Bllink::new(None).await;

    let bllink = match bllink_result {
        Ok(bllink) => {
            print_tagged(Tag::Flash, "found Crazyflie already in bootloader mode");
            bllink
        }
        Err(_) => {
            // 4. Scan for a running Crazyflie and warm-boot it into bootloader mode
            print_tagged(Tag::Flash, "scanning for Crazyflie...");
            let context = LinkContext::new();
            let found = context.scan([0xE7; 5]).await?;
            if found.is_empty() {
                return Err(anyhow!("No Crazyflie found. Make sure it is powered on and in range."));
            }
            let uri = &found[0];
            print_tagged(Tag::Flash, &format!("found {}, rebooting to bootloader", uri));
            let address = warm_boot_to_bootloader(uri).await?;
            print_tagged(Tag::Flash, "in bootloader mode, connecting...");
            Bllink::new(Some(&address)).await?
        }
    };

    // 5. Create the high-level flash interface
    let mut cfloader = cfloader::CFLoader::new(bllink).await?;

    // 6. Determine start address from STM32 bootloader info
    let stm32_info = cfloader.stm32_info();
    let start_address = stm32_info.flash_start() as u32 * stm32_info.page_size() as u32;

    // 7. Flash with progress reporting
    let total = data.len();
    let progress_callback = move |bytes_written: usize, _total: usize| {
        let pct = bytes_written * 100 / total;
        print_tagged(Tag::Flash, &format!("progress {}%", pct));
    };
    cfloader
        .flash_stm32_with_progress(start_address, &data, Some(progress_callback))
        .await?;

    // 8. Reset back to firmware
    cfloader.reset_to_firmware().await?;

    print_tagged(Tag::Flash, "done");
    Ok(())
}
