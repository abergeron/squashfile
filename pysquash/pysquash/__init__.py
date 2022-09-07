from .pysquash import (
    SquashError,
    SquashCursor as _SquashCursor,
)
from io import BufferedReader, TextIOWrapper, DEFAULT_BUFFER_SIZE


class SquashCursor:
    def __init__(self, path):
        if isinstance(path, _SquashCursor):
            self._cur = path
        else:
            self._cur = _SquashCursor(str(path))

    def cd(self, path):
        return SquashCursor(self._cur.cd(bytes(path)))

    def open(
        self, path, binary=False, buffering=-1, encoding=None, errors=None, newline=None
    ):
        raw = self._cur.open(bytes(path))
        if buffering == 0:
            if not binary:
                raise ValueError("no buffering not allowed in text mode")
            return raw
        if buffering == -1 or (buffering == 1 and not binary):
            buffering = DEFAULT_BUFFER_SIZE
        buf = BufferedReader(raw, buffering)
        if binary:
            return buf
        line_buffering = buffering == 1
        return TextIOWrapper(
            buf,
            encoding=encoding,
            errors=errors,
            newline=newline,
            line_buffering=line_buffering,
        )

    def __iter__(self):
        self._cur.scandir()
