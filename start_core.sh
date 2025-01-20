# Change these if running on a different host
COREFILE=$(dirname $0)/coresesh.xml
COREDIR=/tmp/pycore.1
NETLISTENER=/home/ubuntu/network_listener/target/release/network_listener
RUN_MGEN=/home/ubuntu/network_listener/mgensh/run_mgen.sh

NLST_OUTPUT=nlst.log
MGEN_OUTPUT=mgen.log

# Make sure core-daemon is running and that all sessions are deleted
if test -d $COREDIR; then
  echo "ERROR: Directory for pycore.1 exists. Please run core-cli session -i 1 delete"
  exit 1
fi

if ! test -f $COREFILE; then
    echo "ERROR: File $COREFILE does not exist!"
    exit 1
fi

# Start testnet3.xml
core-cli xml -f $COREFILE -s

if ! test -d $COREDIR; then
  echo "ERROR: Failed to create $COREFILE"
  echo "    $COREDIR does not exist or is not a directory"
  exit 1
fi

PIDS=()

# Device hostnames
NLST_DEVS=(pc120 pc121 pc220 pc221 pc320 pc321)

# Device ip addrs
MGEN_IPS=(10.0.1.20 10.0.1.21 10.0.2.20 10.0.2.21 10.0.3.20 10.0.3.21)

# start all the different mgen instances
# The core-cli script will generate session 1
for i in ${!NLST_DEVS[@]}
do
    dev=${NLST_DEVS[i]}
    out=$COREDIR/$dev.conf
    vcmd -c $COREDIR/$dev -- $NETLISTENER >> $out/$NLST_OUTPUT &
    PIDS+=($!)
    echo "Started network_listener on $dev [$!]"
    sleep 0.5

    vcmd -c $COREDIR/$dev -- $RUN_MGEN ${MGEN_IPS[i]} -o $out/$MGEN_OUTPUT &
    PIDS+=($!)
    echo "Started traffic generator on $dev [$!]"
    sleep 0.5
done

# for dev in ${NLST_DEVS[@]}
# do
#     vcmd -c $COREDIR/$dev -- $NETLISTENER >> $COREDIR/$dev.conf/$OUTPUT &
#     LSTNERS+=($!)
#     echo "Started network_listener on $dev [$!]"
#     sleep 0.5
# done

read -p "Press enter to destroy simulation and stuff."

for pid in ${PIDS[@]}
do
    kill -2 $pid
done

core-cli session -i 1 delete

exit 1
