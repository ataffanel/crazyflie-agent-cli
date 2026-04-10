---
name: crazyflie-dev
description: >
  Crazyflie quadcopter firmware development with hardware in the loop.
  Use this skill whenever the user asks you to work on Crazyflie firmware,
  develop features for the Crazyflie, fix bugs in the Crazyflie firmware,
  or interact with a Crazyflie drone in any way. Also use it when you see
  a crazyflie-firmware project in the working directory, or when the user
  mentions quadcopters, drones, STM32 firmware with FreeRTOS in the context
  of Bitcraze products. This skill gives you the ability to flash firmware,
  read sensor data, tune parameters, and verify your changes on real hardware.
---

# Crazyflie Firmware Development

You have access to a Crazyflie quadcopter connected via a Crazyradio USB dongle. The `crazyflie-agent-cli` tool lets you flash firmware, stream sensor data, read/write parameters, and monitor debug output - all over radio. This means you can compile firmware, flash it to the real hardware, and verify it works, in a tight development loop.

## Setup Check

Before starting, verify the CLI is available:

```bash
which crazyflie-agent-cli
```

If the command is not found, **ask the user before installing anything**. Tell them:

> "I need `crazyflie-agent-cli` to interact with the Crazyflie hardware. It can be installed with `cargo install --git https://github.com/ataffanel/crazyflie-agent-cli`. This requires a Rust toolchain. Want me to install it?"

Only proceed with installation if the user agrees. If they don't have Rust installed, point them to https://rustup.rs/ and let them handle it.

## Before You Start

Ask the user for the Crazyflie's radio URI if you don't know it. It looks like `radio://0/80/2M/E7E7E7E7E7` (channel 80, 2Mbps, default address). If they don't know, run `crazyflie-agent-cli scan` to find it. Always use the explicit URI the user gives you rather than relying on scan results, because radio leakage can cause scan to pick up adjacent channels.

## The Development Loop

This is your core workflow. Every firmware change follows this cycle:

1. **Edit** the firmware source code
2. **Build** with `make -j$(nproc)` in the firmware directory
3. **Flash** with `crazyflie-agent-cli flash <path-to-bin> --uri <URI>`
4. **Verify** by reading console output and/or streaming log variables
5. **Iterate** based on what you observe

Keep a session running so you can see console output and log data between flashes.

### Setting Up a Session

Start a background daemon that stays connected to the Crazyflie and streams all output to a file:

```bash
crazyflie-agent-cli start <URI> > /tmp/cf-output.log 2>&1 &
```

This gives you:
- Live console output from the firmware (`[console]` lines)
- Log variable data when logging is active (`[log]` lines)
- Connection status changes (`[status]` lines)

Read the output file whenever you need to check what's happening:

```bash
# Check console output (firmware debug messages)
grep "\[console\]" /tmp/cf-output.log | tail -20

# Check log data
grep "\[log " /tmp/cf-output.log | tail -20

# Check for errors
grep "\[error\]" /tmp/cf-output.log
```

### Building Firmware

The Crazyflie firmware uses Kbuild (Linux kernel build system):

```bash
cd <firmware-directory>
make cf2_defconfig    # Only needed once, or after config changes
make -j$(nproc)       # Build - produces build/cf2.bin
```

If you need to change build configuration (enable/disable features), use `make menuconfig` or edit the `.config` file.

### Flashing

Always specify the URI explicitly:

```bash
crazyflie-agent-cli flash build/cf2.bin --uri radio://0/80/2M/E7E7E7E7E7
```

The flash command will:
- Stop any running daemon session (needs exclusive radio access)
- Reboot the Crazyflie into bootloader mode
- Flash the firmware
- Reboot back to normal mode

After flashing, restart your session to reconnect:

```bash
crazyflie-agent-cli start <URI> > /tmp/cf-output.log 2>&1 &
```

If the Crazyflie is already in bootloader mode (e.g. after a crash recovery), use the `--cold` flag:

```bash
crazyflie-agent-cli flash build/cf2.bin --cold
```

### Reading Parameters

Parameters are runtime-configurable values in the firmware. They're organized as `group.name`:

```bash
crazyflie-agent-cli param list              # List all with current values
crazyflie-agent-cli param get pid_rate.kp   # Read one
crazyflie-agent-cli param set pid_rate.kp 50  # Write one
```

### Streaming Log Variables

Log variables are read-only sensor data and internal state, streamed at a configurable rate:

```bash
# Start logging specific variables at 10 Hz
crazyflie-agent-cli log start stateEstimate.roll stateEstimate.pitch pm.vbat --rate 10

# Data appears in the daemon output file
grep "\[log " /tmp/cf-output.log | tail -10

# Stop logging
crazyflie-agent-cli log stop
```

Use `crazyflie-agent-cli log list` to see all available variables.

### Ending a Session

```bash
crazyflie-agent-cli stop
```

## Adding Log Variables and Parameters to Firmware

This is one of the most common firmware development tasks. The firmware uses C macros to define log variables and parameters at compile time.

### Adding a Log Variable

In the relevant `.c` file:

```c
#include "log.h"

static float myValue;  // The variable to expose

// At the bottom of the file, inside a log group:
LOG_GROUP_START(myModule)
  LOG_ADD(LOG_FLOAT, myVar, &myValue)
LOG_GROUP_STOP(myModule)
```

After building and flashing, `myModule.myVar` will appear in `log list`.

Available types: `LOG_UINT8`, `LOG_UINT16`, `LOG_UINT32`, `LOG_INT8`, `LOG_INT16`, `LOG_INT32`, `LOG_FLOAT`, `LOG_FP16`.

### Adding a Parameter

```c
#include "param.h"

static float myParam = 1.0f;  // Default value

PARAM_GROUP_START(myModule)
  PARAM_ADD(PARAM_FLOAT, myParam, &myParam)
PARAM_GROUP_STOP(myModule)
```

For read-only parameters, use `PARAM_ADD(PARAM_FLOAT | PARAM_RONLY, ...)`.

## Firmware Architecture Quick Reference

The firmware is a FreeRTOS application on an STM32F405. Key directories:

- `src/modules/src/` - Core modules (stabilizer, commander, estimator, log, param)
- `src/modules/interface/` - Public headers
- `src/drivers/src/` - Hardware drivers (IMU, barometer, motors)
- `src/hal/src/` - Hardware abstraction layer
- `src/deck/` - Expansion deck drivers
- `src/platform/` - Platform-specific code (CF2, Bolt, etc.)

Key subsystems:
- **Stabilizer** (`stabilizer.c`) - Main control loop running at 1kHz
- **Commander** (`commander.c`) - Receives setpoints from multiple sources
- **State Estimator** (`estimator_kalman.c`) - Kalman filter for position/attitude
- **CRTP** (`crtp.c`) - Communication protocol over radio

### Useful Log Variables

| Variable | Type | Description |
|----------|------|-------------|
| `stateEstimate.roll/pitch/yaw` | float | Attitude in degrees |
| `stateEstimate.x/y/z` | float | Position estimate (m) |
| `stabilizer.roll/pitch/yaw/thrust` | float | Controller outputs |
| `acc.x/y/z` | float | Accelerometer (g) |
| `gyro.x/y/z` | float | Gyroscope (deg/s) |
| `baro.asl` | float | Barometer altitude (m) |
| `pm.vbat` | float | Battery voltage |
| `sys.canfly` | uint8 | System ready flag |

### Useful Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `stabilizer.controller` | uint8 | Active controller (0=PID, 1=Mellinger) |
| `stabilizer.estimator` | uint8 | Active estimator (0=complementary, 1=Kalman) |
| `pid_rate.kp/ki/kd` | float | Rate PID gains |
| `pid_attitude.kp/ki/kd` | float | Attitude PID gains |

## When Things Go Wrong

### Crazyflie won't connect after flash

The firmware might have crashed before the radio stack initialized. This is the most dangerous failure mode because you lose radio access.

**What to do:**
1. Run `crazyflie-agent-cli recover`
2. If the CLI can still reach the Crazyflie, it will reset it to bootloader mode
3. If not, it will print instructions for the **human** - you need to ask the user to manually put it in bootloader mode (power off, hold button 3 seconds)
4. Once in bootloader mode, flash a known-good firmware with `--cold`

**Ask the user for help** with the message: "The Crazyflie appears to be unresponsive over radio. Could you please put it in bootloader mode? Turn it off, then hold the power button for about 3 seconds until the blue LEDs start blinking. Let me know when it's ready."

### Avoiding radio-bricking

These areas of the firmware are critical for radio communication. Modifying them incorrectly can make the Crazyflie unreachable, requiring physical recovery:

- `src/modules/src/crtp.c` - Communication protocol
- `src/hal/src/radiolink.c` - Radio link layer
- `src/drivers/src/nrf24*` - Radio hardware driver
- `src/init/` - System initialization
- `src/modules/src/system.c` - System startup sequence

If you need to modify any of these files, be extra careful. Consider making a backup flash image first, and warn the user that this change could require manual recovery if something goes wrong.

### Build failures

Common issues:
- Missing toolchain: `arm-none-eabi-gcc` must be installed
- Submodules not initialized: `git submodule update --init --recursive`
- Config not set: run `make cf2_defconfig` first

## Command Reference

| Command | Description |
|---------|-------------|
| `scan` | Find Crazyflies on radio |
| `start <uri>` | Start session, stream to stdout |
| `stop` | End session |
| `status` | Check connection (exit 0/1) |
| `param list` | List all parameters |
| `param get <name>` | Read parameter |
| `param set <name> <val>` | Write parameter |
| `log list` | List all log variables |
| `log start <vars> --rate <hz>` | Stream log data |
| `log stop` | Stop streaming |
| `flash <bin> --uri <uri>` | Flash firmware (warm boot) |
| `flash <bin> --cold` | Flash firmware (already in bootloader) |
| `reset --uri <uri>` | Reboot Crazyflie |
| `recover` | Recovery assistance |
