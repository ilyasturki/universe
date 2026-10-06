"""Over python-dbusmock's NetworkManager: a Wi-Fi card naming the access point it joined, which its template leaves out."""

import dbus
import dbusmock
from dbusmock.templates import networkmanager

FULL = 4
activate_any = networkmanager.activate_connection


@dbus.service.method(dbusmock.MOCK_IFACE, in_signature="ssi", out_signature="s")
def AddWiFiCard(self, device_name, iface_name, state):
    path = self.AddWiFiDevice(device_name, iface_name, state)
    dbusmock.get_object(path).AddProperty(networkmanager.WIRELESS_DEVICE_IFACE, "ActiveAccessPoint", dbus.ObjectPath("/"))
    return path


def activate_on_card(self, conn, dev, ap):
    path = activate_any(self, conn, dev, ap)
    manager = dbusmock.get_object(networkmanager.MANAGER_OBJ)
    if str(dev) != "/" and "ActiveAccessPoint" in dbusmock.get_object(dev).props.get(networkmanager.WIRELESS_DEVICE_IFACE, {}):
        manager.SetProperty(str(dev), networkmanager.WIRELESS_DEVICE_IFACE, "ActiveAccessPoint", dbus.ObjectPath(ap))
    manager.SetConnectivity(FULL)
    return path


def load(_mock, _parameters):
    networkmanager.activate_connection = activate_on_card
    dbusmock.get_object(networkmanager.MANAGER_OBJ).activate_connection = activate_on_card
