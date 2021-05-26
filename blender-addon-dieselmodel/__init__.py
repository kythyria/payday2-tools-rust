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

def import_meshoids_from_file_bmesh(path):
    ts_start = datetime.now()
    meshoids = pd2tools_fdm.get_meshoids_for_filename(path)
    ts_conv = datetime.now()
    for meshoid in meshoids:
        bm = bmesh.new()
        positions = meshoid.position_tuples()
        verts = [bm.verts.new(Vector(v)) for v in positions]
        tri_verts = meshoid.triangle_vertices()
        faces = [bm.faces.new([verts[i] for i in tv]) for tv in tri_verts]

        me = bpy.data.meshes.new("mesh")
        bm.to_mesh(me)
        ob = bpy.data.objects.new("object", me)
        bpy.context.scene.collection.objects.link(ob)
    ts_end = datetime.now()
    print("Loading: {}".format(ts_conv - ts_start))
    print("Importing: {}".format(ts_end - ts_conv))

def import_meshoids_from_file(path):
    ts_start = datetime.now()
    meshoids = pd2tools_fdm.get_meshoids_for_filename(path)
    ts_conv = datetime.now()
    for meshoid in meshoids:
        me = bpy.data.meshes.new("mesh")

        positions = meshoid.position_tuples()
        tri_verts = meshoid.triangle_vertices()
        me.from_pydata(positions, [], tri_verts)

        # This makes the assumption that Mesh doesn't rearrange the faceloops from what
        # was passed as the third argument to from_pydata
        for uv_layer in meshoid.uv_layers:
            uv = me.uv_layers.new(name=uv_layer.name)
            id = uv_layer.data
            for i in range(len(id)):
                uv.data[i].uv = id[i]

        for color_layer in meshoid.colours:
            col = me.vertex_colors.new(name=color_layer.name)
            id = color_layer.data
            for i in range(len(id)):
                col.data[i].color = id[i]
            
        if meshoid.has_normals:
            me.use_auto_smooth = True
            me.create_normals_split()
            norms = meshoid.faceloop_normals()
            for i in range(len(norms)):
                me.loops[i].normal = norms[i]
            me.normals_split_custom_set(norms)

        ob = bpy.data.objects.new("object", me)
        bpy.context.scene.collection.objects.link(ob)
    ts_end = datetime.now()
    print("Loading: {}".format(ts_conv - ts_start))
    print("Importing: {}".format(ts_end - ts_conv))

def import_face(bm, face, loops, verts):
    fv = [verts[loops[i].vertex] for i in face.loops]
    nf = bm.faces.new(fv)