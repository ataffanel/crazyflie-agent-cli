# Crazyflie Agent CLI - Agent Guide

## Overview

`crazyflie-agent-cli` lets you flash firmware, read parameters, stream logs,
and monitor console output from a Crazyflie drone over radio. It uses a
persistent background session so you can issue commands independently.

## Prerequisites

- A Crazyradio PA USB dongle plugged in
- A Crazyflie 2.x powered on and within radio range
- The firmware project at `../crazyflie-firmware/` (or wherever you're developing)

## Quick Start

```bash
# Find your Crazyflie
crazyflie-agent-cli scan

# Start a session (run in background, pipe output to file)
crazyflie-agent-cli start <URI> > /tmp/cf-output.log 2>&1 &

# Now you can issue commands against the running session
crazyflie-agent-cli status
crazyflie-agent-cli param list
crazyflie-agent-cli log start stateEstimate.roll --rate 10

# Read the output file for console and log data
grep "\[console\]" /tmp/cf-output.log
grep "\[log\]" /tmp/cf-output.log | tail -20
```

## Development Loop

The typical firmware development cycle:

1. Edit firmware source code
2. Build: `cd ../crazyflie-firmware && make -j$(nproc)`
3. Flash: `crazyflie-agent-cli flash ../crazyflie-firmware/build/cf2.bin`
4. Monitor: `grep "\[console\]" /tmp/cf-output.log | tail -20`
5. Verify: `crazyflie-agent-cli log start <variables> --rate 10`
6. Read results: `grep "\[log\]" /tmp/cf-output.log | tail -10`
7. Iterate

## Commands

### Session Management
- `start <uri>` - Start background session. Streams [console] and [log] to stdout.
- `stop` - End the session.
- `status` - Check connection. Exit code 0 = connected, 1 = not.

### Parameters
- `param list` - List all parameters with type, access, and current value.
- `param get <name>` - Read a parameter (e.g. `stabilizer.controller`).
- `param set <name> <value>` - Write a parameter.

### Logging
- `log list` - List all log variables with types.
- `log start <var1> [var2...] --rate <hz>` - Start streaming log data.
- `log stop` - Stop logging.

### Firmware
- `flash <path>` - Flash a .bin firmware file. Handles bootloader entry automatically.
- `reset` - Reboot the Crazyflie.

### Recovery
- `recover` - If the Crazyflie is unreachable, guides recovery.

## Output Format

The daemon's stdout uses tagged lines:
- `[console] ...` - Firmware debug output
- `[log 1.234] var1=0.5 var2=1.2` - Log data with timestamp
- `[status] ...` - Connection state changes
- `[flash] ...` - Flash progress
- `[error] ...` - Errors
- `[recover] ...` - Recovery instructions

Filter with grep: `grep "^\[log\]" /tmp/cf-output.log`

## CRITICAL SAFETY WARNING

**If firmware crashes the radio stack, the Crazyflie becomes unreachable over
radio.** Recovery requires physical access: power off, hold button 3 seconds
for bootloader mode. There is no remote recovery.

Avoid modifying:
- CRTP communication code (`src/modules/src/crtp.c`)
- Radio driver code (`src/drivers/src/nrf24*.c`, `src/hal/src/radiolink.c`)
- System initialization (`src/init/`, `src/modules/src/system.c`)

If you must modify these, test very carefully. A bad flash here means asking
the human to manually recover.

## Useful Log Variables

Common variables for debugging:
- `stateEstimate.roll/pitch/yaw` - Attitude in degrees
- `stateEstimate.x/y/z` - Position estimate
- `stabilizer.roll/pitch/yaw/thrust` - Controller outputs
- `pm.vbat` - Battery voltage
- `sys.canfly` - System ready flag

## Useful Parameters

- `stabilizer.controller` - Active controller (0=PID, 1=Mellinger, etc.)
- `stabilizer.estimator` - Active estimator (0=complementary, 1=Kalman)
- `system.arm` - Arm/disarm motors
