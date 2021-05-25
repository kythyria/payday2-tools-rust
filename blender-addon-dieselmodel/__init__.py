from . import pd2tools_fdm

import bpy
import bmesh
from mathutils import *
from datetime import datetime

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
    print("{}: begin read".format(datetime.now()))
    meshoids = pd2tools_fdm.get_meshoids_for_filename(path)
    print("{}: begin convert".format(datetime.now()))
    for meshoid in meshoids:
        bm = bmesh.new()
        verts = [bm.verts.new(Vector(v.co)) for v in meshoid.vertices]
        edges = [bm.edges.new((verts[e.vertices[0]], verts[e.vertices[1]])) for e in meshoid.edges]
        faces = [import_face(bm, f, meshoid.loops, verts) for f in meshoid.faces]

        me = bpy.data.meshes.new("mesh")
        bm.to_mesh(me)
        ob = bpy.data.objects.new("object", me)
        bpy.context.scene.collection.objects.link(ob)
    print("{}: end convert".format(datetime.now()))

def import_face(bm, face, loops, verts):
    fv = [verts[loops[i].vertex] for i in face.loops]
    nf = bm.faces.new(fv)