# Network listener
This project was created as part of a master's thesis at the **University of Oslo**.
The goal was to create a tool passive listening tool, able to passively estimate available bandwidth in an edge environment.

The following instructions will guide you through the installation and setup of the tool, as well as how to run the simulation.

As of writing this, the tool itself is able to run on most Linux distributions, but the simulation part is only tested on **Ubuntu 22.04**.
It should be noted that running the tool outside of an emulation for long periods of time is not
recommended, as it will periodically send **gRPC** hello requests to all seen nodes, which might be considered
malicious behavior.

*It is recommended to install the tool on a virtual machine or a container, as it might be difficult to uninstall.*

## Dependencies
The following script can be used to install the required dependencies for running the `network_listener` binary, however, the tool itself does not do very much alone, and will just send data into the void without any of the infrastructure to receive it.

```bash
# This script will install the packages required to run the tool itself.
sudo apt-get update -y
sudo apt-get install -y \
    libpcap-dev \
    iperf3 \
    net-tools \
    make \
    gcc \
    protobuf-compiler

# Rust installation
cd $HOME
# Check if Rust is installed; if not, install it.
if ! command -v cargo &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi
```

## Running the program
```bash
# Compile release binary.
make build
# Compile and run release.
make run

# Run without compiling.
make runbin
# or
./target/{release|debug}/network_listener
```

# Simulation Instructions
Running the simulation will require:
- CORE Network Emulator
- MGEN packet generator
- PostgreSQL database
- Grafana for visualization
- TimescaleDB for PostgreSQL
- Docker for running the database and Grafana

The following instructions are based on the installation instructions for CORE and MGEN, and the PostgreSQL Docker image.
- CORE installation: [core](https://coreemu.github.io/core/install_ubuntu.html)
- MGEN installation: [mgen](https://github.com/USNavalResearchLaboratory/mgen)
- TimescaleDB installation: [timescaledb](https://docs.timescale.com/self-hosted/latest/install/installation-docker/)

## Installation
If you have docker installed, you need to put the following in /etc/docker/docker.json
```json
{
  "iptables": false
}
```
Alternatively, you can run the following command:
```bash
sudo iptables --policy FORWARD ACCEPT
```

To install CORE and MGEN, you can use the following script:
```bash
# Install all packages in the $HOME directory
./setup.sh
```

## Install simulation dependencies (Ubuntu 22.04)

This includes the installation of the PostgreSQL client, Docker, and other dependencies required for running the simulation, as well as the Rust compiler and Cargo package manager.
```bash
# Install requirements for experimental setup
# Client version 17 should be installed if available, but as
# of writing this guide, it was not present through apt-get.
sudo apt-get update -y
sudo apt-get install -y \
    postgresql-client-common \
    postgresql-client-14 \
    docker \
    libpcap-dev \
    iperf3 \
    net-tools \
    make \
    gcc \
    protobuf-compiler \
    docker.io

# Install rustc and cargo
cd $HOME
# Check if Rust is installed; if not, install it.
if ! command -v cargo &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

# Install and build the project:
cd $HOME
git clone https://github.com/kjetilom/network_listener.git
cd network_listener
make build # cargo build --release
```


# Setting up the database and Grafana
The following instructions will set up a PostgreSQL database with TimescaleDB and Grafana for visualization. Note that passwords and usernames can be changed, but they must be updated in the database_cfg.toml file in the experiments folder.

```bash
# Install Docker image configured with TimescaleDB
# This is required to be able to upload experiment data
sudo docker pull timescale/timescaledb-ha:pg17

# Run the container:
# username, database name and password can be changed
# This should work with the preconfigured
# `database_cfg.toml` file
sudo docker run --name pgrdb \
	  -e POSTGRES_USER=user \
	  -e POSTGRES_PASSWORD=password \
	  -e POSTGRES_DB=metricsdb \
	  -p 5432:5432 \
	  -d timescale/timescaledb-ha:pg17

# Check if the container is running:
sudo docker ps | grep pgrdb

# Try to connect to the database
psql -h localhost -U user -d metricsdb

# Pull the latest Grafana Docker image and run it.
sudo docker run -d \
    -p 3000:3000 \
    --name=grafana \
    grafana/grafana-enterprise
# Accessing http://localhost:3000 should now yield a login page
# with both username and password set to admin.
```

## Configuring Grafana
After installing the dependencies described in Listing \ref{lst:install_reqs}, and starting and configuring the Grafana and PostgreSQL pods as described in Listing \ref{lst:pod_setup}, we can add PostgreSQL as a data source in Grafana. After logging in, navigate to the "Connections" tab, select "Data Sources", and search for and select the "PostgreSQL" data source.

These steps might differ depending on how the pods were configured, but if none were changed, the following should work:
\begin{itemize}
- **Host URL**: Use the IP address of the docker image with the configured port `172.17.0.1:5432`.
- **User/password**: Set to the same as the ones used when starting the PostgreSQL container.
- **TLS/SSL Mode**: Set to disabled
- **TimescaleDB**: Set to enabled
- The rest can be left as default.
- Press the **Save-and-test** button to verify that it works.


If you choose to load the dataset provided, it can be viewed through Grafana by importing the provided JSON file `grafana_dashboard.json` through the option under Dashboards $\rightarrow$ Create dashboard $\rightarrow$ Import dashboard. The visualizations might need to be configured with the correct datasource after loading to show the visualizations, and Grafana shows timeseries data, so the time window needs to be configured according to when the experiment was performed (An example would be Experiment 1, which was started on 2025-04-21 16:18:00).


## Configuring the database
To be able to view results in Grafana, you need to set up the database schema.
This can either be done by loading the experiment data sql dump, or by running the up.sql and views.sql files in the src/scheduler folder.
```bash
# Install git-lfs and gzip to install the dataset-gzip file
sudo apt-get install -y \
    git-lfs \
    gzip

# Clone the tool repository
cd $HOME
git clone https://github.com/kjetilom/network_listener.git

# Install the dataset file.
# This will also sync the folder with all of the plots and figures.
cd network_listener
git lfs pull

# Extract and load the dataset
gzip -d experiment_data.sql.gz
psql -h localhost -U user -d metricsdb < experiment_data.sql
```

### Alternative:
```bash
# This only sets up the database schema for experiment data.
cd network_listener
psql -h localhost -U user -d metricsdb # Enter password

# In the psql shell:
\i src/scheduler/up.sql
\i src/scheduler/views.sql
```

# Running the simulation.
This requires sudo privileges.
The run_experiment.sh script will start a tmux session with different windows for the different components of the simulation.
```bash
# Run the simulation
./run_experiment.sh ./experiments/exp1

# In another terminal, if you want to interact with the emulation:
tmux a -t core
```
CRTL + B, 3 to switch to the node terminal view
It starts 4 terminals, so not every single node, here you can run commands
such as ping, iperf3, etc. to test the network. (It will however impact the results of the experiment)

To stop the simulation, press enter in the window with the experiment running, and wait
for it to finish.


# Creating plots
The `dataplotter.py` script can be used to create plots from the data in the database.
It is highly inefficient, and is provided only to show how plots were created for the thesis. Plots from the experiment data are provided in the `plots_and_figures.tar.xz` tarball.

The contents of views.sql are the foundation for the queries used in the script.
```bash
# Set the correct database configuration in the code itself prior to running the script.
python3 dataplotter.py
```