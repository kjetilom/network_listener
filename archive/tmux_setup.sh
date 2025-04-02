#! /bin/bash
BASE_DIR=$(dirname $0)

# Check if file is passed as an argument
CORESESSION_FILE=$1
if [ -z "$CORESESSION_FILE" ]; then
    echo "Usage: $0 <core_session_file>"
    exit 1
fi

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

# Start the start_core.sh script in the start_core window
tmux send-keys -t "=core:=start_core" "./start_core.sh $CORESESSION_FILE" Enter
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

# How do we stop the session?
# Wait for the user to press enter
read -p "Press enter to stop the session"

# First send enter to the start_core window
tmux send-keys -t "=core:=start_core" Enter
sleep 3

# Stop the core-daemon
tmux send-keys -t "=core:=daemon" C-c
sleep 1
# Stop the tmux session
tmux kill-session -t core
