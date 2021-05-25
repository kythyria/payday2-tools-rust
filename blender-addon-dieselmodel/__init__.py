from . import fmd_read
from . import diesel_hash
from datetime import datetime
from . import pd2tools_fdm

def test2(s):
    return pd2tools_fdm.diesel_hash(s)

def test():
    started = datetime.now()
    f = open("F:\\code\\pd2tools-rust\\hashlist", "rb")
    lines = f.readlines()
    hashes = { diesel_hash.hash(x): x for x in lines }
    print(len(hashes))
    f.close()
    ended = datetime.now()
    print(ended - started)

bl_info = {
    "name": "Diesel model",
    "description": "Reads and writes the model format used by Payday 2",
    "author": "Kyth Tieran",
    "version": (1, 0),
    "blender": (2, 92),
    "category": "Import-Export"
}