#! /bin/bash

# Change these if running on a different host
BASE_DIR=$(dirname $0)
COREFILE=$BASE_DIR/coresesh1.xml

# Absolute path to the directory where the core-daemon will create the session
COREDIR=/tmp/pycore.1

# Get the full path to the executables for the network listener and mgen
# This is needed as vcmd will run the script in the directory of the device
NETLISTENER=$(realpath $BASE_DIR/target/release/network_listener)
SCHEDULER=$(realpath $BASE_DIR/target/release/scheduler)
MGEN_SCRIPTS=$(realpath $BASE_DIR/mgensh/mgen_scripts)

# Output files (relative to the device directory)
NLST_OUTPUT=nlst.log
MGEN_OUTPUT=mgen.log


MGEN=$(which mgen)

# Make sure core-daemon is running and that all sessions are deleted
if test -d $COREDIR; then
  echo "ERROR: Directory for pycore.1 exists. Please run core-cli session -i 1 delete"
  exit 1
fi

if ! test -f $COREFILE; then
    echo "ERROR: File $COREFILE does not exist!"
    exit 1
fi

if ! test -x $NETLISTENER; then
    echo "ERROR: Network listener $NETLISTENER does not exist or is not executable"
    exit 1
fi

if ! test -f $MGEN; then
    echo "ERROR: MGEN $MGEN does not exist"
    exit 1
fi

if ! test -d $MGEN_SCRIPTS; then
    echo "ERROR: MGEN scripts directory $MGEN_SCRIPTS does not exist"
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

    script="$MGEN_SCRIPTS/${MGEN_IPS[i]}.mgn"

    if ! test -f $script; then
        echo "ERROR: No mgen script $script exists, skipping $dev"
        continue
    fi

    vcmd -c $COREDIR/$dev -- $MGEN input $script output $out/$MGEN_OUTPUT &
    PIDS+=($!)
    echo "Started traffic generator on $dev [$!]"
    sleep 0.5
done

# # Start the scheduler server
# vcmd -c $COREDIR/mdr4 -- $SCHEDULER "0.0.0.0:50041" >> $COREDIR/mdr4.conf/$NLST_OUTPUT &
# PIDS+=($!)

read -p "Press [ENTER] to kill started processes and delete emulation session."

# Cleanup started processes.
echo "Killing processes"
for pid in ${PIDS[@]}
do
    kill -2 $pid
done

# Delete the emulation session
echo "Deleting session 1"
core-cli session -i 1 delete

if test -d $COREDIR; then
  echo "ERROR: Failed to delete session 1. 'Please run core-cli session -i 1 delete' manually"
  exit 1
fi

echo "Done"
exit 0
