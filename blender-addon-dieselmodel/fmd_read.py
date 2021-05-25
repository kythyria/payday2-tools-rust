import struct
from typing import Dict, Tuple, List

# I can't believe I have to write this
class BinaryReader:
    def __init__(self, buf) -> None:
        self.buffer = buf
        self.position: int = 0
        pass

    def seek(self, pos: int) -> None:
        self.position = pos
    
    def skip(self, offset:int) -> None:
        self.position += offset

    def read_f32(self) -> int:
        self.read("<f")[0]

    def read_u32(self) -> int:
        self.read("<I")[0]
    
    def read_u64(self) -> int:
        self.read("<Q")[0]

    def read(self, pattern: str) -> tuple:
        count = struct.calcsize(pattern)
        result = struct.unpack_from(pattern, self.buffer, self.position)
        self.position += count
        return result

    def read_slice(self, length:int) -> "BinaryReader":
        start = self.position
        end = self.position + length
        sub_buffer = self.buffer[start:end]
        self.position = end
        return BinaryReader(sub_buffer)

def read_file_header(br: BinaryReader) -> Tuple[int, int]:
    section_count = br.read_u32()
    file_length = br.read_u32()
    if section_count == 0xFFFFFFFF:
        section_count = br.read_u32()
    return (section_count, file_length)

def read_file(br: BinaryReader) -> Dict[int, object]:
    output = {}

    header = read_file_header(br)
    section_count = header[0]
    for c in range(section_count):
        (type_tag, id, length) = br.read("<III")
        body = br.read_slice(length)
        if type_tag in fmd_parsers:
            output[id] = fmd_parsers[type_tag](body)
    return output

class Object3d():
    name: int
    controllers: List[int]
    transform: Tuple[float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float] #can't use the real type, Matrix, because no typestubs for blender
    parent: int

    def __init__(self) -> None:
        pass

    def load(self, br: BinaryReader):
        self.name = br.read_u64()
        controller_count = br.read_u32()
        self.controllers = []
        for i in range(controller_count):
            self.controllers.append(br.read_u32())
            br.skip(8)
        self.transform = br.read("<ffffffffffffffff")

class BoundsModel(Object3d):
    bounds_min: Tuple[float, float, float]
    bounds_max: Tuple[float, float, float]
    unknown_7: float
    unknown_8: int

    def __init__(self) -> None:
        super().__init__()
        pass

    def load(self, br: BinaryReader):
        super().load(br)
        self.bounds_min = br.read("<fff")
        self.bounds_max = br.read("<fff")
        self.unknown_7 = br.read_f32()
        self.unknown_8 = br.read_u32()

class RenderAtom:
    base_vertex: int
    triangle_count: int
    base_index: int
    geometry_slice_length: int
    material_id: int

    def load(self, br: BinaryReader):
        self.base_vertex = br.read_u32()
        self.triangle_count = br.read_u32()
        self.base_index = br.read_u32()
        self.geometry_slice_length = br.read_u32()
        self.material_id = br.read_u32()


class GeometricModel(Object3d):
    geometry_provider: int
    index_provider: int
    renderatoms: List[RenderAtom]
    material_group: int
    lightset: int
    properties: int
    bounds_min: Tuple[float, float, float]
    bounds_max: Tuple[float, float, float]
    bounds_radius: float
    unknown_13: float
    skinbones: int

    def load(self, br: BinaryReader):
        ra_count = br.read_u32()
        for i in range(ra_count):
            ra = RenderAtom()
            ra.load(br)
            self.renderatoms.append(ra)
        self.material_group = br.read_u32()
        self.lightset = br.read_u32()
        self.properties = br.read_u32()
        self.bounds_min = br.read("<fff")
        self.bounds_max = br.read("<fff")
        self.bounds_radius = br.read_f32()
        self.unknown_13 = br.read_u32()
        self.skinbones = br.read_u32()

class Geometry:
    pass

def parse_model(br:BinaryReader):
    type = br.read_u32()
    obj = None
    if type == 6:
        obj = BoundsModel()
    else:
        obj = GeometricModel()
    obj.load(br)
    return obj

def parse_object3d(br: BinaryReader):
    obj = Object3d()
    obj.load(br)
    return obj

fmd_parsers = {
    0x0FFCD100: parse_object3d,
    0x62212D88: parse_model,
#    0x7AB072D3: fmd_read_geometry,
#    0x4C507A13: fmd_read_topology,
#    0xE3A3B1CA: fmd_read_passthroughgp,
#    0x03B634BD: fmd_read_topologyip
}