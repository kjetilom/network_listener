#! /bin/bash
###----------------------------------------------------
### Verify experiment folder and set up environment
###----------------------------------------------------

# Parse command line arguments
# experiment : path to the experiment folder
EXPERIMENT=$1
if [ -z "$EXPERIMENT" ]; then
    echo "Usage: $0 <experiment_folder>"
    exit 1
fi

# Check if the experiment folder exists
if [ ! -d "$EXPERIMENT" ]; then
    echo "ERROR: Experiment folder $EXPERIMENT does not exist!"
    exit 1
fi

# Check for experiment/setenv.sh
if [ ! -f "$EXPERIMENT/setenv.sh" ]; then
    echo "ERROR: Configuration file $EXPERIMENT/setenv.sh does not exist!"
    exit 1
else
    # Source the setenv.sh file to set up the environment
    source "$EXPERIMENT/setenv.sh"
fi

echo $CORESESSION
echo $NETWORK_LISTENER_CONFIG
echo $MGEN_SCRIPTS

COREDIR=/tmp/pycore.1
NETLISTENER=$(realpath $(dirname $0)/target/release/network_listener)
echo $NETLISTENER

# Output files (relative to the device directory)
NLST_OUTPUT=nlst.log
MGEN_OUTPUT=mgen.log

# Create temporary directory for log files
if [ ! -d "$EXPERIMENT/tmp" ]; then
    echo "Creating temporary directory $EXPERIMENT/tmp"
    mkdir -p "$EXPERIMENT/tmp"
fi

MGEN=$(which mgen)
if [ -z "$MGEN" ]; then
    echo "ERROR: MGEN not found in PATH!"
    exit 1
fi

# Make sure core-daemon is running and that all sessions are deleted
if test -d $COREDIR; then
  echo "ERROR: Directory for pycore.1 exists. Please run core-cli session -i 1 delete"
  exit 1
fi

if ! test -f $CORESESSION; then
    echo "ERROR: File $COREFILE does not exist!"
    exit 1
fi

###----------------------------------------------------
### Set up TMUX session and start core-daemon
###----------------------------------------------------

BASE_DIR=$(dirname $0)

# Create a new tmux session
tmux new-session -d -s core -c $BASE_DIR
# Create a new window for the daemon
tmux new-window -d -t "=core" -n daemon -c $BASE_DIR
# Create a new window for start_core.sh
tmux new-window -d -t "=core" -n start_core -c $BASE_DIR

# Start the core-daemon in the daemon window
tmux send-keys -t "=core:=daemon" "sudo core-daemon" Enter

# This takes some time to start, so we will wait for a few seconds
sleep 3

###----------------------------------------------------
### Start emulation session and network listener
###----------------------------------------------------

# Run the core-session
core-cli xml -f $CORESESSION -s

if ! test -d $COREDIR; then
  echo "ERROR: Failed to create $CORESESSION"
  echo "    $COREDIR does not exist or is not a directory"
  exit 1
fi

# Store the process IDs of the started processes
PIDS=()

for i in ${!NODES[@]}
do
    # Split the string into IP and name
    IFS=' ' read -r ip name <<< "${NODES[i]}"

    echo "Starting network_listener on $name [$ip]"

    out_dir="$EXPERIMENT/tmp/$name"
    if [ ! -d "$out_dir" ]; then
        echo "Creating output directory $out_dir"
        mkdir -p "$out_dir"
    fi
    # Start the network listener
    vcmd -c $COREDIR/$name -- $NETLISTENER -c $NETWORK_LISTENER_CONFIG >> $out_dir/$NLST_OUTPUT &
    PIDS+=($!)

    mgen_script="$MGEN_SCRIPTS/${ip}.mgn"
    # Verify script existence
    if [ ! -f "$mgen_script" ]; then
        echo "ERROR: MGEN script $mgen_script does not exist!"
        exit 1
    fi

    # Start the MGEN traffic generator
    vcmd -c $COREDIR/$name -- $MGEN input $mgen_script output /dev/null & # $out_dir/$MGEN_OUTPUT
    PIDS+=($!)
    echo "Started MGEN on $name [$!]"

    # Disable segment offloading
    vcmd -c $COREDIR/$name -- ethtool --offload eth0 rx off tx off
    echo "Disabled segment offloading on $name"
done

###----------------------------------------------------
### Post core setup: Initialize tmux windows for ease of use
###----------------------------------------------------

# Start the start_core.sh script in the start_core window
sleep 5

tmux new-window -d -t "=core" -n nodes -c $BASE_DIR

tmux split-window -h -t "=core:=nodes.0" -c $BASE_DIR
tmux split-window -v -t "=core:=nodes.0" -c $BASE_DIR
tmux split-window -v -t "=core:=nodes.2" -c $BASE_DIR

# Run virtual shell in each pane
tmux send-keys -t "=core:=nodes.0" "vcmd -c /tmp/pycore.1/pc120" Enter
tmux send-keys -t "=core:=nodes.1" "vcmd -c /tmp/pycore.1/pc220" Enter
tmux send-keys -t "=core:=nodes.2" "vcmd -c /tmp/pycore.1/pc320" Enter
tmux send-keys -t "=core:=nodes.3" "vcmd -c /tmp/pycore.1/pc221" Enter
sleep 3



###----------------------------------------------------
### Wait for user input to kill processes and delete session
###----------------------------------------------------

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

# Ask the user if they want to delete the temporary directory
read -p "Do you want to delete the temporary directory $EXPERIMENT/tmp? (y/n) " answer
if [[ $answer == "y" || $answer == "Y" ]]; then
    rm -rf "$EXPERIMENT/tmp"
    echo "Deleted temporary directory $EXPERIMENT/tmp"
else
    echo "Temporary directory $EXPERIMENT/tmp not deleted"
fi

# Stop the core-daemon
tmux send-keys -t "=core:=daemon" C-c
sleep 1

# Stop the tmux session
tmux kill-session -t core

echo "Killed processes and deleted session"
exit 0

### End of script
