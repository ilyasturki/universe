from typing import TYPE_CHECKING, Any, TypeVar

# PySide6 registers these by name and no Python type maps to them: object becomes PyObjectWrapper, int is 32-bit
QVARIANT: Any = "QVariant"
QULONGLONG: Any = "qulonglong"

T = TypeVar("T")

if TYPE_CHECKING:
    from collections.abc import Callable

    # PySide6's stub types a Property as the descriptor itself; read through an instance it is what the getter returns
    def Property(
        type: Any,
        fget: Callable[[Any], T],
        fset: Callable[[Any, Any], Any] | None = None,
        freset: Callable[[Any], Any] | None = None,
        fdel: Callable[[Any], Any] | None = None,
        doc: str = "",
        notify: Any = None,
        designable: bool = True,
        scriptable: bool = True,
        stored: bool = True,
        user: bool = False,
        constant: bool = False,
        final: bool = False,
    ) -> T: ...
else:
    from PySide6.QtCore import Property

__all__ = ["QULONGLONG", "QVARIANT", "Property"]
