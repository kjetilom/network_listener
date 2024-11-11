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


## Creating a monitor interface
```bash
# Creating a monitor interface
# Note, the interface names may vary (e.g. phy0, phy1, etc.)

# If needed, several monitor interfaces can be created for different phy
# interfaces if available

sudo iw dev
sudo iw phy interface add mon0 type monitor
sudo iw phy0 interface add mon0 type monitor
sudo ifconfig mon0 up
```