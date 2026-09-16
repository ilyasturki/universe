# `args` is `(kind, message)`, as `universe_core.UniverseError`'s.
class UniverseError(Exception):
    def __init__(self, kind, message):
        super().__init__(kind, message)
        self.kind = kind
        self.message = message
