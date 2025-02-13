# Network listener
A simple network sniffing tool for monitoring network traffic and analyzing packets, tcp connections, link utilization, etc.

## Running the program
```bash
# Compile the program
make
# Compile and run the program
make run

# Run without compiling
make runbin
# or
./target/{release|debug}/network_listener
```

# Simulation
Running the simulation will require:
- CORE Network Emulator
- MGEN packet generator
As well as a lot of other packages.

For installing on ubunutu 22.04:
```bash

```
You have to be root to run the script.
Information about uninstalling can be found below:
[core](https://coreemu.github.io/core/)
[mgen](https://github.com/USNavalResearchLaboratory/mgen)

## Installation
If you have docker installed, you need to put the following in /etc/docker/docker.json
```json
{
  "iptables": false
}
```

```bash
# Install all packages in the $HOME directory
./setup.sh

# Install rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the project (This will build in release mode)
make build
```

## Running the simulation.
This requires that you don't need to input password for sudo commands.
```bash
# Run the simulation
./tmux_setup.sh

# In another terminal
tmux a -t core
# CRTL + B, 3 to switch to the node terminal view
```