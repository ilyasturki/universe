import os
import struct

UHID = "/dev/uhid"

UHID_DESTROY, UHID_START, UHID_STOP, UHID_OPEN, UHID_CLOSE, UHID_OUTPUT = 1, 2, 3, 4, 5, 6
UHID_GET_REPORT, UHID_GET_REPORT_REPLY, UHID_CREATE2, UHID_INPUT2, UHID_SET_REPORT, UHID_SET_REPORT_REPLY = 9, 10, 11, 12, 13, 14
UHID_EVENT_SIZE = 4380
UHID_FEATURE_REPORT, UHID_OUTPUT_REPORT, UHID_INPUT_REPORT = 0, 1, 2
BUS_USB, BUS_BLUETOOTH = 0x03, 0x05
EIO = 5


class Device:
    """A HID device over /dev/uhid, which the kernel's drivers take for one on a cable or Bluetooth: what they ask of it comes back
    as events, what it reports goes in as input reports."""

    def __init__(self, name, uniq, descriptor, bus, vendor, product, version=0, country=0, path=UHID):
        self.fd = os.open(path, os.O_RDWR | os.O_CLOEXEC)
        create = struct.pack("<I128s64s64sHHIIII", UHID_CREATE2, name.encode(), b"", uniq.encode(), len(descriptor), bus, vendor, product, version, country)
        os.write(self.fd, create + descriptor)

    def input(self, report):
        os.write(self.fd, struct.pack("<IH", UHID_INPUT2, len(report)) + report)

    def event(self) -> tuple[int, tuple]:
        """The next request: (UHID_GET_REPORT, (request, number, kind)), (UHID_SET_REPORT, (request, number, kind, data)),
        (UHID_OUTPUT, (data, kind)), or any other event with ()."""
        buf = os.read(self.fd, UHID_EVENT_SIZE)
        event = struct.unpack_from("<I", buf)[0]
        if event == UHID_GET_REPORT:
            return event, struct.unpack_from("<IBB", buf, 4)
        if event == UHID_SET_REPORT:
            request, number, kind, size = struct.unpack_from("<IBBH", buf, 4)
            return event, (request, number, kind, buf[12 : 12 + size])
        if event == UHID_OUTPUT:
            size, kind = struct.unpack_from("<HB", buf, 4 + 4096)
            return event, (buf[4 : 4 + size], kind)
        return event, ()

    def reply_get(self, request, data=None):
        """`data` the report asked for, report number first; None: the device has no such report."""
        if data is None:
            os.write(self.fd, struct.pack("<IIHH", UHID_GET_REPORT_REPLY, request, EIO, 0))
        else:
            os.write(self.fd, struct.pack("<IIHH", UHID_GET_REPORT_REPLY, request, 0, len(data)) + bytes(data))

    def reply_set(self, request, error=0):
        os.write(self.fd, struct.pack("<IIH", UHID_SET_REPORT_REPLY, request, error))

    def close(self):
        try:
            os.write(self.fd, struct.pack("<I", UHID_DESTROY))
        finally:
            os.close(self.fd)
