# crazyflie-agent-cli

A CLI tool that enables AI agents to do hardware-in-the-loop Crazyflie firmware development. Flash firmware, stream sensor data, read/write parameters, and monitor debug output - all over radio.

## Install

Requires a Rust toolchain ([rustup.rs](https://rustup.rs/)).

```bash
cargo install --git https://github.com/ataffanel/crazyflie-agent-cli
```

## Quick Start

```bash
# Find your Crazyflie
crazyflie-agent-cli scan

# Start a session (streams console + log data to stdout)
crazyflie-agent-cli start radio://0/80/2M/E7E7E7E7E7 > /tmp/cf-output.log 2>&1 &

# Interact
crazyflie-agent-cli status
crazyflie-agent-cli param list
crazyflie-agent-cli log start stateEstimate.roll stateEstimate.pitch --rate 10

# Flash firmware
crazyflie-agent-cli flash build/cf2.bin --uri radio://0/80/2M/E7E7E7E7E7
```

## Install the Claude Code Skill

The `crazyflie-dev` skill teaches an AI agent how to use this CLI for firmware development.

### Via Claude Code plugin (recommended)

In Claude Code, first add the marketplace:

```
/plugin marketplace add ataffanel/crazyflie-agent-cli
```

Then install the plugin:

```
/plugin install crazyflie-dev@crazyflie-agent-cli
```

### Manual install

Copy the skill into your firmware project:

```bash
mkdir -p <your-firmware-project>/.claude/skills/crazyflie-dev
curl -o <your-firmware-project>/.claude/skills/crazyflie-dev/SKILL.md \
  https://raw.githubusercontent.com/ataffanel/crazyflie-agent-cli/master/skills/crazyflie-dev/SKILL.md
```

Once installed, Claude Code will automatically use the skill when you ask it to work on Crazyflie firmware.

## Architecture

The CLI uses a daemon/client architecture:

- **`start`** launches a foreground daemon that connects to the Crazyflie, streams console/log data to stdout, and listens on a Unix socket for commands
- **All other commands** are thin clients that send JSON requests to the daemon over the socket
- **`flash`**, **`scan`**, **`reset`**, and **`recover`** operate independently (no daemon needed)

## License

MIT OR Apache-2.0
