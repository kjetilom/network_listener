# install system packages
sudo apt-get update -y
sudo apt-get install -y \
    ca-certificates \
    xterm \
    psmisc \
    python3 \
    python3-tk \
    python3-pip \
    python3-venv \
    wget \
    iproute2 \
    iputils-ping \
    tcpdump

# install ospf mdr
cd $HOME
sudo apt-get install -y \
    automake \
    gawk \
    g++ \
    libreadline-dev \
    libtool \
    make \
    pkg-config \
    git
git clone https://github.com/USNavalResearchLaboratory/ospf-mdr.git
cd ospf-mdr
./bootstrap.sh
./configure --disable-doc --enable-user=root --enable-group=root \
    --with-cflags=-ggdb --sysconfdir=/usr/local/etc/quagga --enable-vtysh \
    --localstatedir=/var/run/quagga
make -j$(nproc)
sudo make install

# install emane
cd $HOME
EMANE_RELEASE=emane-1.5.1-release-1
EMANE_PACKAGE=${EMANE_RELEASE}.ubuntu-22_04.amd64.tar.gz
wget -q https://adjacentlink.com/downloads/emane/${EMANE_PACKAGE}
tar xf ${EMANE_PACKAGE}
cd ${EMANE_RELEASE}/debs/ubuntu-22_04/amd64
rm emane-spectrum-tools*.deb emane-model-lte*.deb
rm *dev*.deb
sudo apt-get install -y ./emane*.deb ./python3-emane_*.deb

# install core
cd $HOME
CORE_PACKAGE=core_9.2.0_amd64.deb
PACKAGE_URL=https://github.com/coreemu/core/releases/latest/download/${CORE_PACKAGE}
wget -q ${PACKAGE_URL}
sudo apt-get install -y ./${CORE_PACKAGE}

# install emane python bindings
cd $HOME
sudo apt-get install -y \
    unzip \
    libpcap-dev \
    libpcre3-dev \
    libprotobuf-dev \
    libxml2-dev \
    protobuf-compiler \
    uuid-dev
wget https://github.com/protocolbuffers/protobuf/releases/download/v3.19.6/protoc-3.19.6-linux-x86_64.zip
mkdir protoc
unzip protoc-3.19.6-linux-x86_64.zip -d protoc
git clone https://github.com/adjacentlink/emane.git
cd emane
git checkout v1.5.1
./autogen.sh
./configure --prefix=/usr
cd src/python
PATH=~/protoc/bin:$PATH make
sudo /opt/core/venv/bin/python -m pip install .

# install MGEN
cd $HOME
git clone https://github.com/USNavalResearchLaboratory/mgen.git
cd mgen
git submodule update --init
cd makefiles
make -f Makefile.linux
