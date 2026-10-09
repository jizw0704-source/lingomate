"""Windows-only COM factory smoke checks; never register a source or access a host."""

import ctypes
import sys
import unittest
import uuid
from pathlib import Path


class Guid(ctypes.Structure):
    _fields_ = [("data", ctypes.c_ubyte * 16)]

    @classmethod
    def from_text(cls, text):
        return cls.from_buffer_copy(uuid.UUID(text).bytes_le)


class DllTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dll = ctypes.WinDLL(str(Path(sys.argv[1]).resolve()))
        cls.dll.DllGetClassObject.argtypes = [
            ctypes.POINTER(Guid),
            ctypes.POINTER(Guid),
            ctypes.POINTER(ctypes.c_void_p),
        ]
        cls.dll.DllGetClassObject.restype = ctypes.c_int32
        cls.dll.DllCanUnloadNow.restype = ctypes.c_int32

    def test_factory_lifetime_and_unknown_class(self):
        iid = Guid.from_text("00000001-0000-0000-c000-000000000046")
        missing = Guid.from_text("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")
        pointer = ctypes.c_void_p()
        self.assertLess(
            self.dll.DllGetClassObject(
                ctypes.byref(missing), ctypes.byref(iid), ctypes.byref(pointer)
            ),
            0,
        )
        self.assertFalse(pointer.value)
        self.assertEqual(self.dll.DllCanUnloadNow(), 0)
        clsid = Guid.from_text("078a5202-3f7a-4dd5-89cc-322a9011df81")
        self.assertEqual(
            self.dll.DllGetClassObject(
                ctypes.byref(clsid), ctypes.byref(iid), ctypes.byref(pointer)
            ),
            0,
        )
        self.assertTrue(pointer.value)
        self.assertEqual(self.dll.DllCanUnloadNow(), 1)
        table = ctypes.cast(pointer, ctypes.POINTER(ctypes.POINTER(ctypes.c_void_p)))[0]
        release = ctypes.WINFUNCTYPE(ctypes.c_uint32, ctypes.c_void_p)(table[2])
        self.assertEqual(release(pointer), 0)
        self.assertEqual(self.dll.DllCanUnloadNow(), 0)


if __name__ == "__main__":
    if sys.platform != "win32":
        raise SystemExit("This smoke test requires Windows; it does not run on macOS.")
    unittest.main(argv=[sys.argv[0]])
