# Get a list of IP addresses for the current machine
addrs=$(ifconfig | grep -Eo 'inet (addr:)?([0-9]*\.){3}[0-9]*' | grep -Eo '([0-9]*\.){3}[0-9]*' | grep -v '127.0.0.1')

# The list is currently a string, turn it into a list of ip addrs:
addrs=($addrs)
echo "IP addresses: ${addrs[@]}"


# Define function
run_mgen() {
    echo "Running mgen on $1"
    # if mgen_scripts/mgen_script_$2.txt exists, run it
    if [ -f mgen_scripts/$2.mgn ]; then
        echo "Running mgen script mgen_scripts/$2.mgn"
    else
        echo "No mgen script mgen_scripts/$2.mgn"
        return
    fi
    mgen input mgen_scripts/$2.mgn
}

# Define a functions to be run based on the IP address
for addr in ${addrs[@]}; do
    run_mgen $addr $addr
done