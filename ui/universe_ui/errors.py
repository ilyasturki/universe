class UniverseError(Exception):
    """The core's failure as the frontend sees it: `args` is `(kind, message)`, like `universe_core.UniverseError`."""

    def __init__(self, kind, message):
        super().__init__(kind, message)
        self.kind = kind
        self.message = message
