# !/bin/bash

CORESESSION=$(realpath "$EXPERIMENT/config/core_session.xml")
NETWORK_LISTENER_CONFIG=$(realpath "$EXPERIMENT/config/nlst_cfg.toml")
MGEN_SCRIPTS=$(realpath "$EXPERIMENT/mgen_scripts")
EXPERIMENT_DESCRIPTION=""
EXPERIMENT_NAME="exp2_fluid"

# Nodes in the format 'IP NAME'
# NAME is the name of the node in the core session
NODES=(
    '10.0.1.20 pc120'
    '10.0.1.21 pc121'
    '10.0.2.20 pc220'
    '10.0.2.21 pc221'
    '10.0.3.20 pc320'
    '10.0.3.21 pc321'
)




