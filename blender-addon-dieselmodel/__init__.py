from . import pd2tools_fdm

import bpy
import bmesh
from mathutils import *
from datetime import datetime
import struct

bl_info = {
    "name": "Diesel model",
    "description": "Reads and writes the model format used by Payday 2",
    "author": "Kyth Tieran",
    "version": (1, 0),
    "blender": (2, 92),
    "category": "Import-Export"
}

def hash(s):
    return pd2tools_fdm.diesel_hash(s)

def import_meshoids_from_file(path):
    ts_start = datetime.now()
    meshoids = pd2tools_fdm.get_meshoids_for_filename(path)
    ts_conv = datetime.now()
    for meshoid in meshoids:
        bm = bmesh.new()
        positions = meshoid.position_tuples
        verts = [bm.verts.new(Vector(v)) for v in positions]
        tri_verts = meshoid.triangle_vertices
        faces = [bm.faces.new([verts[i] for i in tv]) for tv in tri_verts]

        me = bpy.data.meshes.new("mesh")
        bm.to_mesh(me)
        ob = bpy.data.objects.new("object", me)
        bpy.context.scene.collection.objects.link(ob)
    ts_end = datetime.now()
    print("Loading: {}".format(ts_conv - ts_start))
    print("Importing: {}".format(ts_end - ts_conv))

def import_face(bm, face, loops, verts):
    fv = [verts[loops[i].vertex] for i in face.loops]
    nf = bm.faces.new(fv)