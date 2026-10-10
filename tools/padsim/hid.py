"""A HID report descriptor read the way the kernel's hid-input reads it: each input report's fields, where they sit and what they
stand for, so a report can be built field by field."""

from dataclasses import dataclass, field

GENERIC_DESKTOP, SIMULATION, BUTTON, CONSUMER = 0x01, 0x02, 0x09, 0x0C
CONSTANT, VARIABLE, NULL_STATE = 0x01, 0x02, 0x40


@dataclass
class Field:
    report: int
    offset: int  # bits, after the report id
    size: int
    count: int
    usages: list  # (page << 16) | usage, one per element of a Variable field
    lo: int
    hi: int
    flags: int


@dataclass
class Descriptor:
    inputs: dict = field(default_factory=dict)  # report id → [Field]
    sizes: dict = field(default_factory=dict)  # report id → bits after the id
    application: int = 0

    def field_of(self, usage):
        """The report, field and element that carry `usage`."""
        for report, fields in self.inputs.items():
            for f in fields:
                if f.flags & VARIABLE and usage in f.usages:
                    return report, f, f.usages.index(usage)
        return None


def _signed(value, size):
    bits = size * 8
    return value - (1 << bits) if size and value & (1 << (bits - 1)) else value


def parse(data):
    out = Descriptor()
    globals_ = {"page": 0, "lo": 0, "hi": 0, "size": 0, "count": 0, "report": 0}
    stack = []
    usages, usage_min = [], None
    at = 0
    while at < len(data):
        prefix = data[at]
        if prefix == 0xFE:
            at += 3 + data[at + 1]
            continue
        size = (0, 1, 2, 4)[prefix & 3]
        kind, tag = (prefix >> 2) & 3, prefix >> 4
        raw = int.from_bytes(data[at + 1 : at + 1 + size], "little")
        at += 1 + size
        if kind == 1:
            if tag == 0x0:
                globals_["page"] = raw
            elif tag == 0x1:
                globals_["lo"] = _signed(raw, size)
            elif tag == 0x2:
                globals_["hi"] = _signed(raw, size)
                if globals_["hi"] < globals_["lo"]:
                    globals_["hi"] = raw
            elif tag == 0x7:
                globals_["size"] = raw
            elif tag == 0x8:
                globals_["report"] = raw
            elif tag == 0x9:
                globals_["count"] = raw
            elif tag == 0xA:
                stack.append(dict(globals_))
            elif tag == 0xB and stack:
                globals_ = stack.pop()
            continue
        if kind == 2:
            full = raw if size == 4 else (globals_["page"] << 16) | raw
            if tag == 0x0:
                usages.append(full)
            elif tag == 0x1:
                usage_min = full
            elif tag == 0x2 and usage_min is not None:
                usages.extend(range(usage_min, full + 1))
                usage_min = None
            continue
        if kind == 0:
            if tag == 0xA and not out.application and usages:
                out.application = usages[0]
            if tag == 0x8:
                report, bits, count = globals_["report"], globals_["size"], globals_["count"]
                offset = out.sizes.get(report, 0)
                flags = raw
                elements = [usages[min(i, len(usages) - 1)] for i in range(count)] if usages and flags & VARIABLE else list(usages)
                out.inputs.setdefault(report, []).append(Field(report, offset, bits, count, elements, globals_["lo"], globals_["hi"], flags))
                out.sizes[report] = offset + bits * count
            usages, usage_min = [], None
    return out


def put(report, f, element, value):
    """Writes `value` into element `element` of field `f`, in a report whose byte 0 is the report id when the descriptor has ids."""
    start = f.offset + element * f.size + (8 if f.report else 0)
    value &= (1 << f.size) - 1
    for bit in range(f.size):
        byte, shift = divmod(start + bit, 8)
        if value >> bit & 1:
            report[byte] |= 1 << shift
        else:
            report[byte] &= ~(1 << shift)
