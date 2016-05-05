
import ctypes

def _get_libdehtml():
    if _get_libdehtml.value is not None:
        return _get_libdehtml.value
    libdehtml = ctypes.CDLL('../capi/target/debug/libdehtml_capi.so')
    libdehtml.parse_html.argtypes = (
        ctypes.c_char_p, ctypes.c_size_t,
        ctypes.c_char_p, ctypes.c_size_t
    )
    libdehtml.parse_html.restype = ctypes.c_int
    libdehtml.dehtml_errstr.argtypes = (ctypes.c_int, )
    libdehtml.dehtml_errstr.restype = ctypes.c_char_p
    _get_libdehtml.value = libdehtml
    return libdehtml


_get_libdehtml.value = None


class BadRecordType(Exception):
    pass


class Truncated(Exception):
    pass


class DeHtmlError(Exception):
    def __init__(self, errno):
        self.errno = errno
        super(DeHtmlError, self).__init__(_get_libdehtml().dehtml_errstr(errno))


def parse_html(val, bufsize=None):
    if bufsize is None:
        bufsize = 4096 * 4
    ibuf = ctypes.create_string_buffer(val.encode('utf8'))
    obuf = ctypes.create_string_buffer(bufsize)
    rv = _get_libdehtml().parse_html(ibuf, len(ibuf), obuf, len(obuf))
    if rv < 0:
        raise DeHtmlError(rv)
    return obuf.value[:rv]


def struct_read(format, fh):
    size = struct.calcsize(format)
    buf = fh.read(size)
    if len(buf) < size:
        raise Truncated
    return struct.unpack(format, buf)


def get_documents(fh):
    while True:
        (record_type, thread_id) = struct_read('!IQ', fh)
        if record_type not in (0, 1):
            raise BadRecordType
        if record_type == 0:
            (value_len, ) = struct_read('!Q', fh)
            buf = fh.read(value_len)
            if len(buf) < value_len:
                raise Truncated
            yield (thread_id, buf)
