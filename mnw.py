#!/usr/bin/env python

'This example creates a simple network topology with 1 AP and 2 stations'

import sys

from mininet.log import setLogLevel, info
from mn_wifi.cli import CLI
from mn_wifi.net import Mininet_wifi
from mn_wifi.manetRoutingProtocols import batmand
from mn_wifi.link import wmediumd, adhoc
from mn_wifi.wmediumdConnector import interference
from typing import Dict
from mn_wifi.node import Station

class WirelessNode:
    def __init__(self, name):
        pass

class WirelessMesh:
    def __init__(self, net: Mininet_wifi):
        self.net = net
        self.stations: Dict[str, Station] = {}

    def set_default(self, **kwargs):
        self.default = kwargs

    def set_link_default(self, **kwargs):
        self.link_default = kwargs

    def add_station(self, name, cls=None, **kwargs):
        return self.net.addStation(name, cls, **kwargs, **self.default)

    def add_link(self, ):
        pass




def topology():
    "Create a network."
    net = Mininet_wifi(link=wmediumd, wmediumd_mode=interference)

    mesh = WirelessMesh(net=net)
    mesh.set_default()

    info("*** Creating nodes\n")
    sta_args = {
        "mode": "g",
        "channel": 5,
        "ssid": "adhocNet"
    }

    # if '-v' in sys.argv:
    #     sta_arg = {'nvif': 2}
    # else:
    #     # isolate_clientes: Client isolation can be used to prevent low-level
    #     # bridging of frames between associated stations in the BSS.
    #     # By default, this bridging is allowed.
    #     # OpenFlow rules are required to allow communication among nodes
    #     ap_arg = {'client_isolation': True}

    sta1 = net.addStation('sta1', position="10,10,0", **sta_args, range=50, client_isolation=True)
    sta2 = net.addStation("sta2", position="50,10,0", **sta_args, range=50, client_isolation=True)
    sta3 = net.addStation("sta3", position="90,10,0", **sta_args, range=50, client_isolation=True)
    # sta4 = net.addStation("sta4", position="90,50,0", **sta_args, range=50)
    # sta5 = net.addStation("sta5", position="90,90,0", **sta_args, range=50)

    net.setPropagationModel(model="logDistance", exp=4)

    info("*** Configuring nodes\n")
    net.configureNodes()

    info("*** Associating Stations\n")
    net.addLink(sta1, cls=adhoc, intf='sta1-wlan0',
            ssid='adhocNet', proto="batmand",
            mode='g', channel=5)
    net.addLink(sta2, cls=adhoc, intf='sta2-wlan0',
            ssid='adhocNet', proto="batmand",
            mode='g', channel=5)
    net.addLink(sta3, cls=adhoc, intf='sta3-wlan0',
            ssid='adhocNet', proto="batmand",
            mode='g', channel=5)
    # net.addLink(sta4, cls=adhoc, intf='sta4-wlan0',
    #         ssid='adhocNet', proto="batmand",
    #         mode='g', channel=5, ht_cap='HT40+')
    # net.addLink(sta5, cls=adhoc, intf='sta5-wlan0',
    #         ssid='adhocNet', proto="batmand",
    #         mode='g', channel=5, ht_cap='HT40+')

    net.plotGraph(max_x=200, max_y=200)

    info("*** Starting network\n")
    net.build()

    info("*** Running CLI\n")
    CLI(net)

    info("*** Stopping network\n")
    net.stop()


if __name__ == '__main__':
    setLogLevel('info')
    topology()
