from core.api.grpc import client
from core.api.grpc.wrappers import NodeType, Position, SessionState, ConfigOption, Node

# interface helper
iface_helper = client.InterfaceHelper(ip4_prefix="10.0.0.0/24", ip6_prefix="2001::/64")

# create grpc client and connect
core = client.CoreGrpcClient()
core.connect()

# add session
try:
    session = core.get_session(1337)
    print("Got session from existing")
    initialized = True
except:
    session = core.create_session(1337)
    print("created new session")
    initialized = False


core.start_session(session)


def initialize():
    # create nodes
    position = Position(x=200, y=200)
    wlan = session.add_node(1,name="WirelessNet", _type=NodeType.WIRELESS_LAN, position=position)
    wlan.set_wlan(
        {
            "range": "280",
            "bandwidth": "1234567",
            "delay": "6000",
            "jitter": "5",
            "error": "5",
        }
    )

    position = Position(x=100, y=100)
    node1 = session.add_node(2, name="MDR1", model="mdr", position=position)
    position = Position(x=300, y=100)
    node2 = session.add_node(3, name="MDR2", model="mdr", position=position)
    position = Position(x=400, y=150)
    node3 = session.add_node(4, name="MDR3", model="mdr", position=position)

    # create links
    iface1 = iface_helper.create_iface(node1.id, 0)
    session.add_link(node1=node1, node2=wlan, iface1=iface1)
    iface1 = iface_helper.create_iface(node2.id, 0)
    session.add_link(node1=node2, node2=wlan, iface1=iface1)
    iface1 = iface_helper.create_iface(node3.id, 0)
    session.add_link(node1=node3, node2=wlan, iface1=iface1)



    # start session



if not initialized:
    initialize()

if session.state != SessionState.RUNTIME:
    print("*** Starting session")
    core.start_session(session)