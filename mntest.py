"""Custom topology example

Two directly connected switches plus a host for each switch:

   host --- switch --- switch --- host

Adding the 'topos' dict with a key/value pair to generate our newly defined
topology enables one to pass in '--topo=mytopo' from the command line.
"""

from mininet.topo import Topo
from mininet.link import TCLink

class MyTopo( Topo ):
    "Simple topology example."

    def build( self ):
        "Create custom topo."

        # Add hosts and switches
        leftHost = self.addHost( 'h1' )
        rightHost = self.addHost( 'h2' )
        leftSwitch = self.addSwitch( 's3' )
        rightSwitch = self.addSwitch( 's4' )

        # Add links
        self.addLink( leftHost, leftSwitch, bw=1000, delay='1ms', use_tbf=True )
#        self.addLink( leftHost, rightHost, bw=10, delay='10ms', loss=10, use_tbf=False )
        self.addLink( leftSwitch, rightSwitch, bw=100, delay='10ms',loss=1.0, use_tbf=True)
        self.addLink( rightSwitch, rightHost, bw=1000, delay='1ms', use_tbf=True )



topos = { 'extop': ( lambda: MyTopo() ) }
