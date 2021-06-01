from . import pd2tools_fdm

import bpy
from bpy.props import (StringProperty,
                        BoolProperty,
                        EnumProperty,
                        IntProperty,
                        CollectionProperty)
from bpy_extras.io_utils import ImportHelper
from bpy.types import Operator
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

def import_ir_from_file(hlp, path):
    ts_start = datetime.now()
    ir_objects = pd2tools_fdm.import_ir_from_file(hlp, path)
    ts_conv = datetime.now()

    mat_dict = {}
    b_objects = {}
    for obj in ir_objects:
        data = data_from_ir(obj.name, obj.data, mat_dict)
        ob = bpy.data.objects.new(obj.name, data)

        bpy.context.scene.collection.objects.link(ob)
        b_objects[obj] = ob
    
    bpy.context.view_layer.update()

    for obj in ir_objects:
        if not obj.parent is None:
            b_objects[obj].parent = b_objects[obj.parent]
            bpy.context.view_layer.update()
        
        loc, rot, sca = Matrix(obj.transform).decompose()
        print(obj.name, rot)
        b_objects[obj].location = (loc.x, loc.y, loc.z)
        b_objects[obj].rotation_mode = "QUATERNION"
        b_objects[obj].rotation_quaternion = (rot.w, rot.x, rot.y, rot.z)
        b_objects[obj].scale = (sca.x, sca.y, sca.z)
    ts_end = datetime.now()
    print("Loading: {}".format(ts_conv - ts_start))
    print("Importing: {}".format(ts_end - ts_conv))

def data_from_ir(name, data, mats):
    if data is None:
        return None
    elif data.data_type == "MESH":
        return mesh_from_ir(name, data, mats)
    elif data.data_type == "BOUNDS":
        return bounds_from_ir(name, data)
    else:
        raise Exception("Unrecognised data type")

def bounds_from_ir(name, data):
    bmax = data.box_max
    bmin = data.box_min
    verts = [
        (bmax[0], bmax[1], bmax[2]),
        (bmax[0], bmin[1], bmax[2]),
        (bmax[0], bmax[1], bmin[2]),
        (bmax[0], bmin[1], bmin[2]),

        (bmin[0], bmax[1], bmax[2]),
        (bmin[0], bmin[1], bmax[2]),
        (bmin[0], bmax[1], bmin[2]),
        (bmin[0], bmin[1], bmin[2])
    ]
    edges = [
        (0, 1), (1, 3),
        (0, 2), (2, 3),

        (4, 5), (5, 7),
        (4, 6), (6, 7),

        (0, 4), (1, 5), (2, 6), (3, 7)
    ]
    me = bpy.data.meshes.new(name)
    me.from_pydata(verts, edges, [])
    return me

def mesh_from_ir(name, data, mats):
    me = bpy.data.meshes.new(name)

    for mn in data.material_names:
        if not mn in mats:
            mats[mn] = bpy.data.materials.new(mn)
        me.materials.append(mats[mn])
    
    me.from_pydata(data.vert_positions, data.edges, data.faces)

    #edge_flags = data.edge_flags
    #for i in range(len(edge_flags)):
    #    e = me.edges[i]
    #    e.use_edge_sharp = edge_flags[i][0]
    #    e.use_seam = edge_flags[i][1]

    face_materials = data.face_materials
    for i in range(len(face_materials)):
        me.polygons[i].material_index = face_materials[i]

    if data.has_normals:
        for f in me.polygons:
            f.use_smooth = True
        me.use_auto_smooth = True
        me.create_normals_split()
        norms = data.loop_normals
        me.normals_split_custom_set(norms)

    for (name, uvs) in data.loop_uv_layers:
        uv = me.uv_layers.new(name=name)
        for i in range(len(uvs)):
            uv.data[i].uv = uvs[i]

    for (name, colours) in data.loop_colour_layers:
        col = me.vertex_colors.new(name=name)
        for i in range(len(colours)):
            col.data[i].color = colours[i]
    
    return me

class Pd2toolsPreferences(bpy.types.AddonPreferences):
    bl_idname = __name__
    hashlist_path: StringProperty(
        name="Hashlist location",
        description="Hashlist to use when importing models. The usual format: one string per line, LF line endings.",
        subtype='FILE_PATH'
    )

    def draw(self, context):
        layout = self.layout
        layout.prop(self, "hashlist_path")


class ImportDieselModel(bpy.types.Operator, ImportHelper):
    """Read from a Diesel .model file"""
    bl_idname = "import.pd2diesel"
    bl_label = "Import Diesel model"
    bl_options = {'REGISTER', 'UNDO'}

    filter_glob: StringProperty(default="*.model", options={'HIDDEN'})
    filter_glob: StringProperty(
	        default="*.model",
	        options={'HIDDEN'},
	        maxlen=1024
	    )
    
    def execute(self, context):
        preferences = context.preferences
        addon_prefs = preferences.addons[__name__].preferences

        import_ir_from_file(addon_prefs.hashlist_path, self.filepath)
        return {'FINISHED'}

def menu_func_import(self, context):
    self.layout.operator(ImportDieselModel.bl_idname, text="Diesel Model (.model)")

def register():
    bpy.utils.register_class(Pd2toolsPreferences)
    bpy.utils.register_class(ImportDieselModel)
    bpy.types.TOPBAR_MT_file_import.append(menu_func_import)

def unregister():
    bpy.types.TOPBAR_MT_file_import.remove(menu_func_import)
    bpy.utils.unregister_class(ImportDieselModel)
    bpy.utils.unregister_class(Pd2toolsPreferences)
