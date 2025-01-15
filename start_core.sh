# Make sure core-daemon is running and that all sessions are deleted
if test -d /tmp/pycore.1; then
  echo "ERROR: Directory for pycore.1 exists. Please run core-cli session -i 1 delete"
  exit 1
fi

# Start testnet3.xml
core-cli xml -f coresesh.xml -s

LSTNERS=()
# start all the different mgen instances
# The core-cli script will generate session 1
vcmd -c /tmp/pycore.1/pc120 -- /home/ubuntu/network_listener/target/release/network_listener >> /tmp/pycore.1/pc120.conf/log.log &
LSTNERS+=($!)
echo ${LSTNERS[@]}
sleep 1
vcmd -c /tmp/pycore.1/pc320 -- /home/ubuntu/network_listener/target/release/network_listener >> /tmp/pycore.1/pc320.conf/log.log &
LSTNERS+=($!)
echo ${LSTNERS[@]}
sleep 1
vcmd -c /tmp/pycore.1/pc220 -- /home/ubuntu/network_listener/target/release/network_listener >> /tmp/pycore.1/pc220.conf/log.log &
LSTNERS+=($!)
echo ${LSTNERS[@]}
sleep 1
vcmd -c /tmp/pycore.1/pc321 -- /home/ubuntu/network_listener/target/release/network_listener >> /tmp/pycore.1/pc321.conf/log.log &
LSTNERS+=($!)
echo ${LSTNERS[@]}
sleep 1

read -p "Press enter to destroy simulation and stuff."

for pid in ${LSTNERS[@]}
do
    kill -2 $pid
done

core-cli session -i 1 delete

exit 1

# core-gui
