from . import pd2tools_fdm

import bpy

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
    meshoids = pd2tools_fdm.get_meshoids_for_filename(path)
    for meshoid in meshoids:
        me = bpy.data.meshes.new("Mesh")
        me.vertices.add(len(meshoid.vertices))
        for i in range(len(meshoid.vertices)):
            me.vertices[i].co = meshoid.vertices[i].co
        ob = bpy.data.objects.new("Object", me)

