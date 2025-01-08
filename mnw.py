#!/usr/bin/env python

'This example creates a simple network topology with 1 AP and 2 stations'

import sys

from mininet.log import setLogLevel, info
from mn_wifi.topo import Topo
from mn_wifi.cli import CLI
from mn_wifi.net import Mininet_wifi
from mn_wifi.manetRoutingProtocols import batmand
from mn_wifi.link import wmediumd, adhoc
from mn_wifi.wmediumdConnector import interference
from typing import Dict
from mn_wifi.node import Station
from mininet.term import tunnelX11
import os
import signal
import time

terms = []


def opern_terminal(self, node, title, geometry, cmd="bash"):
    display, tunnel = tunnelX11(node)

    return node.popen(
        ["xterm",
         "-hold",
         "-title", f" {title} ",
         "-geometry", geometry,
         "-display", display,
         "-e", 'env TERM=ansi %s' % cmd
         ])

class WirelessExample(Mininet_wifi):
    "Simple wireless topo for example usage"

    def __init__(self,):
        Mininet_wifi.__init__(self, link=wmediumd, wmediumd_mode=interference)
        self.is_configured = False
        self.setPropagationModel(model="logDistance", exp=4)
        self.sta_args = {
            "mode": "g",
            "channel": 5,
            "ssid": "adhocNet",
            "client_isolation": True,
        }
        self.link_args = {
            "cls": adhoc,
            "ssid": "adhocNet",
            "proto": "batmand",
            "mode": "g",
            "channel": 5,
        }

    def _configure(self):
        info("*** Creating nodes\n")

        sta1 = self.add_node("sta1", pos="40,40,0", range=50)
        sta2 = self.add_node("sta2", pos="80,40,0", range=50)
        sta3 = self.add_node("sta3", pos="120,40,0", range=50)
        #sta4 = self.add_node("sta4", pos="40,80,0", range=50)
        self.configureNodes()

        info("*** Adding links\n")
        self.add_link(sta1)
        self.add_link(sta2)
        self.add_link(sta3)
        #self.add_link(sta4)

    def _plot(self):
        info("*** Plotting graph!\n")
        self.plotGraph(max_x=200, max_y=200)

    def run_all(self):
        self._configure()
        self._plot()
        self.build()

    def add_node(self, name, pos, range):
        return self.addStation(name, position=pos, range=range, **self.sta_args)

    def add_link(self, node):
        return self.addLink(node, intf=f"{node.name}-wlan0", **self.link_args)

def add_node(self, line):
    info(line)
    net: WirelessExample = self.mn
    sta4 = net.addStation(
        "sta4",
        position="90,100,0",
        ssid="adhocNet",
        channel=5,
        mode="g",
        range=50,
        client_isolation=True
    )
    net.addLink(sta4, cls=adhoc, intf='sta4-wlan0',
                ssid='adhocNet', proto="batmand",
                mode='g', channel=5)

def run(self, line):
    "Create a network."

    info("*** Starting network!\n")
    self.mn.run_all()


def do_open(self: CLI, line):
    mn: WirelessExample = self.mn
    for i, node in enumerate(mn.stations):
        y = 0
        if i%4 == 0 and i >1:
            y += 300
        terms.append(opern_terminal(
            self,
            node=node,
            title=node.name,
            geometry=f"80x20+{(550*i)%(550*3)}+{y}",
            cmd="bash",
        ))

setLogLevel('info')
CLI.do_add_node = add_node
CLI.do_open = do_open

mn = WirelessExample()
info("*** Starting network!\n")
mn.run_all()
CLI(mn)
for t in terms:
    os.kill(t.pid, signal.SIGKILL)
mn.stop()