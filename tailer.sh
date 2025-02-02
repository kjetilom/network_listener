#! /bin/bash
# Script for tailing a specific core file

rootdir=/tmp/pycore.1
env_ext=.conf

# Example folder: /tmp/pycore.1/pc120.conf/ (Home directory for the instance)

# Get input parameters
instance=$1
file=$2

# Check if the number of input parameters is correct
if [ $# -ne 2 ]; then
    echo "Usage: tailer.sh <instance> <file>"
    exit 1
fi

# Input: Instance name (e.g. pc120), what to tail (e.g nlst.log)

# Check if /tmp/pycore.1 exists
if [ ! -d $rootdir ]; then
    echo "Root directory $rootdir does not exist, please ensure that the core-daemon is running and that the session is created"
    exit 1
fi

path=$rootdir/$instance$env_ext
file=$path/$file

# Check if the instance path exists
if [ ! -d $path ]; then
    echo "Instance path $path does not exist"
    exit 1
fi

# Check if the file exists
if [ ! -f $file ]; then
    echo "File $file does not exist"
    exit 1
fi

# Tail the file
tail -f $file
