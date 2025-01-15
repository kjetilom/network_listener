#!/usr/bin/env bash
#
# generate-commands.sh
# Usage: ./generate-commands.sh <remote_ip_1> <remote_ip_2> ...
#
# This script:
# 1. Detects your local IP (assumes a single primary interface).
# 2. Accepts remote IPs as parameters.
# 3. Outputs relevant LISTEN/ON commands based on local or remote IP logic.

# Detect local IP address (grabs first non-loopback IPv4)
LOCAL_IP=$(hostname -I | awk '{print $1}')

# Read optional remote IPs from command line
REMOTE_IPS=("$@")

# Print banner
echo "#--- Automatic Command Generator ---#"
echo "# Local node IP: $LOCAL_IP"
echo "# Remote IPs: ${REMOTE_IPS[*]}"
echo

# Start listening on UDP and TCP ports
echo "LISTEN UDP 5001-5004"
echo "LISTEN TCP 5020,5021"
echo

counter=1
# For each remote IP addr start 1 tcp connection which randomly performs bursts
for ip in $REMOTE_IPS
do
    $((counter++))
    echo "ON $counter UDP DST $ip/"
done

# Example logic to generate 'ON' commands:
# - If this node is 10.0.0.2 (just an example condition),
#   use REMOTE_IP_1 for one set of commands; otherwise, use a different set.
# - Adjust the logic and conditions as needed.

if [ "$LOCAL_IP" = "10.0.0.2" ]; then
  # For local IP 10.0.0.2, assume first remote IP is 10.0.0.3
  REMOTE_1="${REMOTE_IPS[0]:-10.0.0.3}"
  REMOTE_2="${REMOTE_IPS[1]:-10.0.0.1}"

  echo "ON 1 UDP DST $REMOTE_1/5001 PERIODIC [5 8192]"
  echo "ON 2 UDP DST $REMOTE_2/5001 PERIODIC [5 8192]"
  echo
  echo "ON 3 TCP DST $REMOTE_1/5031 BURST [RANDOM 10.0 PERIODIC [10.0 8192] EXP 5.0] RETRY -1/5"
  echo "ON 4 TCP DST $REMOTE_2/5011 BURST [RANDOM 10.0 PERIODIC [10.0 8192] EXP 5.0] RETRY -1/5"
else
  # Default behavior for other nodes
  REMOTE_1="${REMOTE_IPS[0]:-10.0.0.1}"
  REMOTE_2="${REMOTE_IPS[1]:-10.0.0.2}"

  echo "ON 1 UDP DST $REMOTE_1/5001 PERIODIC [5 8192]"
  echo "ON 2 UDP DST $REMOTE_2/5001 PERIODIC [5 8192]"
  echo
  echo "ON 3 TCP DST $REMOTE_1/5031 BURST [RANDOM 10.0 PERIODIC [10.0 8192] EXP 5.0] RETRY -1/5"
  echo "ON 4 TCP DST $REMOTE_2/5011 BURST [RANDOM 10.0 PERIODIC [10.0 8192] EXP 5.0] RETRY -1/5"
fi

# End of file