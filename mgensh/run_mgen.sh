#!/usr/bin/env bash
OUTPUT="mgen.log"
MGEN_SCRIPTS="mgen_scripts"
IPADDRS=()
# chdir to relative script location
cd $(dirname "$0")
echo $0




while (( "$#" )); do
    case "$1" in
        -o)
            OUTPUT="$2"
            shift 2
            ;;
        -*)
            echo "ERROR: Unknown option: $1"
            exit 1
            ;;
        *)
            IPADDRS+=("$1")
            shift
            ;;
    esac
done

pids=()

# Define a functions to be run based on the IP address
for addr in ${IPADDRS[@]}; do
    script="$MGEN_SCRIPTS/$addr.mgn"
    if ! test -f $script; then
        echo "No mgen script $script exists"
        exit 1
    fi
    mgen input $script output $OUTPUT & >> $OUTPUT
    pids+=($!)
done

do_exit () {
    echo ${pids[@]}
    for pid in ${pids[@]}; do
        kill -2 $pid
    done
    exit 0
}
trap do_exit INT

sleep infinity