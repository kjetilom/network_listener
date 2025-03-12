#!/bin/bash

# Get list of all network interfaces except loopback
interfaces=$(ls /sys/class/net | grep -v lo)

# Disable offloading features on each interface
for iface in $interfaces; do
    echo "Disabling offloading on $iface"
    ethtool -K $iface tso off gso off gro off lro off rx off tx off
done

