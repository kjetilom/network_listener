Steps to enable monitor mode on wireless card:
```bash
sudo ifconfig # Find wireless interface name
sudo ip link set NAME down
sudo iw NAME set monitor none
sudo ip link set NAME up

# Check if monitor mode is enabled
sudo iw dev
```