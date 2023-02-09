from . import pd2tools_fdm

import bpy
from bpy.props import (StringProperty,
                        BoolProperty,
                        EnumProperty,
                        IntProperty,
                        CollectionProperty,
                        PointerProperty)
from bpy_extras.io_utils import ImportHelper, ExportHelper
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

BINARY_VERSION = pd2tools_fdm.LIB_VERSION

def hash(s):
    return pd2tools_fdm.diesel_hash(s)

def import_ir_from_file(hlp, path, units_per_cm, framerate):
    ts_start = datetime.now()
    ir_objects = pd2tools_fdm.import_ir_from_file(hlp, path, units_per_cm, framerate)
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
        b_objects[obj].location = (loc.x, loc.y, loc.z)
        b_objects[obj].rotation_mode = "QUATERNION"
        b_objects[obj].rotation_quaternion = (rot.w, rot.x, rot.y, rot.z)
        b_objects[obj].scale = (sca.x, sca.y, sca.z)

        apply_anims(obj, b_objects[obj])
    ts_end = datetime.now()
    print("Loading: {}".format(ts_conv - ts_start))
    print("Importing: {}".format(ts_end - ts_conv))

def apply_anims(src, dest):
    if len(src.animations) == 0:
        return
    
    dest.animation_data_create()
    action = bpy.data.actions.new(dest.name)
    action.id_root = src.data_type
    dest.animation_data.action = action
    for chan in src.animations:
        curve = action.fcurves.new(chan.target_path, index=chan.target_index)
        for (ts, v) in chan.fcurve:
            kf =  curve.keyframe_points.insert(ts, v, options={"NEEDED"})
            kf.handle_left_type = "VECTOR"
            kf.handle_right_type = "VECTOR"


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
    author_tag: StringProperty(
        name="Default author tag",
        description="When exporting models, set the author tag to this if nothing else is specified",
        default="nemo@erehwon.invalid"
    )
    version_string = f"Pd2Tools binary version: {BINARY_VERSION}"

    def draw(self, context):
        layout = self.layout
        layout.label(text=self.version_string)
        layout.prop(self, "hashlist_path")
        layout.prop(self, "author_tag")


class ImportDieselModel(bpy.types.Operator, ImportHelper):
    """Read from a Diesel .model file"""
    bl_idname = "import.pd2diesel"
    bl_label = "Import Diesel model"
    bl_options = {'REGISTER', 'UNDO'}

    filter_glob: StringProperty(
            default="*.model",
            options={'HIDDEN'},
            maxlen=1024
        )
    
    def execute(self, context):
        preferences = context.preferences
        addon_prefs = preferences.addons[__name__].preferences

        # it seems that this is in metres per unit, so rearrange it to the other
        # way around
        metres_per_unit = context.scene.unit_settings.scale_length
        cm_per_unit = metres_per_unit * 100
        units_per_cm = 1/cm_per_unit

        fps = context.scene.render.fps / context.scene.render.fps_base

        import_ir_from_file(addon_prefs.hashlist_path, self.filepath, units_per_cm, fps)
        return {'FINISHED'}

class ExportOilModel(bpy.types.Operator, ExportHelper):
    """Write to an Overkill OIL file"""
    bl_idname = "export.overkill_oil"
    bl_label = "Export OIL model"
    bl_options = {'REGISTER', 'UNDO'}

    filename_ext = ".model"
    filter_glob: StringProperty(
        default="*.model",
        options={'HIDDEN'},
        maxlen=1024
    )

    """
    ABC123
    123ABC
    A1B2C3
    1A2B3C
    """

    """
    A B C 1 2 3
    0 1 2 3 4 5
    3 4 5 0 1 2
    0 1 2 0 1 2
    0 2 4 1 3 5
    1 3 5 0 2 4
    """

   #vcol_numbering_strategy: EnumProperty(
   #    name="Vertex alpha numbering",
   #    description="How to number the alpha attributes of vertex colours",
   #    default="SAME",
   #    items=[
   #        ("SAME", "Same", "Alpha attributes have the same index as the matching RGB attributes"),
   #        ("INTERLEAVED_AFTER", "Interleaved (After)", "Alpha attributes have the index following that of the RGB attribute" ),
   #        ("INTERLEAVED_BEFORE", "Interleaved (Before)", "Alpha attributes numbered after the matching RGB attributes"),
   #    ]
   #)

   #vcol_placement_strategy: EnumProperty(
   #    name="Vertex alpha ordering",
   #    description="Where to place vertex alpha attributes relative to RGB attributes",
   #    default="SORTED",
   #    items=[
   #        ("SORTED_RGB_FIRST", "Sorted (RGB first)", "In numerical order, RGB wins ties"),
   #        ("SORTED_ALPHA_FIRST", "Sorted (alpha first)", "Numerical order, alpha wins ties"),
   #        ("BLOCK_RGB_FIRST", "Block (RGB first)", "All the RGB, then all the alpha"),
   #        ("BLOCK_RGB_LAST", "Block (alpha first)", "All the alpha, then all the RGB")
   #    ]
   #)

    def execute(self, context):
        addon_prefs = context.preferences.addons[__name__].preferences

        metres_per_unit = context.scene.unit_settings.scale_length
        fps = context.scene.render.fps / context.scene.render.fps_base

        pd2tools_fdm.export_oil(self.filepath, metres_per_unit, addon_prefs.author_tag, bpy.context.active_object, self.vcol_numbering_strategy)
        return {'FINISHED'}

class DieselSceneSettings(bpy.types.PropertyGroup):
    override_author_tag: BoolProperty(name="Override author tag", default=False)
    author_tag: StringProperty(name="Author Tag")
    override_source_path: BoolProperty(name="Override source path", default=False)
    source_path: StringProperty(name="Source path")
    scene_type: StringProperty(name="OIL scene type", default="default")

    @classmethod
    def register(cls):
        bpy.types.Scene.diesel = PointerProperty(
            name="Diesel scene settings",
            type=cls
        )
    
    @classmethod
    def unregister(cls):
        del bpy.types.Scene.diesel

class DIESEL_PT_settings_scene(bpy.types.Panel):
    bl_idname = "DIESEL_PT_settings_scene"
    bl_space_type = "PROPERTIES"
    bl_region_type = "WINDOW"
    bl_context = "scene"
    bl_options = {'DEFAULT_CLOSED'}
    bl_label = "Diesel Export"

    def draw(self, ctx):
        layout = self.layout
        layout.use_property_split = True
        layout.use_property_decorate = False

        diesel = ctx.scene.diesel

        atr = layout.row(heading="Override author")
        atr.prop(diesel, "override_author_tag", text="")
        atrs = atr.row()
        atrs.active = diesel.override_author_tag
        atrs.prop(diesel, "author_tag", text="")

        spr = layout.row(heading="Override filepath")
        spr.prop(diesel, "override_source_path", text="")
        sprs = spr.row()
        sprs.active = diesel.override_source_path
        sprs.prop(diesel, "source_path", text="")

        layout.prop(diesel, "scene_type")

class DieselMeshSettings(bpy.types.PropertyGroup):
    cast_shadows: BoolProperty(name="Cast shadows", default=True)
    receive_shadows: BoolProperty(name="Receive shadows", default=True)
    bounds_only: BoolProperty(name="Export as bounds", default=False)

    @classmethod
    def register(cls):
        bpy.types.Mesh.diesel = PointerProperty(
            name="Diesel mesh settings",
            type=cls
        )
    
    @classmethod
    def unregister(cls):
        del bpy.types.Mesh.diesel

class DIESEL_PT_settings_mesh(bpy.types.Panel):
    bl_space_type = "PROPERTIES"
    bl_region_type = "WINDOW"
    bl_context = "data"
    bl_options = {'DEFAULT_CLOSED'}
    bl_label = "Diesel Export"

    @classmethod
    def poll(cls, ctx):
        return ctx.object.type in {'MESH','CURVE','SURFACE','FONT','META'}

    def draw(self, ctx):
        layout = self.layout
        layout.use_property_split = True
        layout.use_property_decorate = False

        layout.prop(ctx.object.data.diesel, "cast_shadows")
        layout.prop(ctx.object.data.diesel, "receive_shadows")
        layout.prop(ctx.object.data.diesel, "bounds_only")

def menu_func_import(self, context):
    self.layout.operator(ImportDieselModel.bl_idname, text="Diesel Model (.model)")

def menu_func_export(self, context):
    self.layout.operator(ExportOilModel.bl_idname, text="Overkill OIL Model (.oil)")

def register():
    bpy.utils.register_class(Pd2toolsPreferences)
    bpy.utils.register_class(ImportDieselModel)
    bpy.utils.register_class(ExportOilModel)
    bpy.utils.register_class(DieselSceneSettings)
    bpy.utils.register_class(DieselMeshSettings)
    bpy.utils.register_class(DIESEL_PT_settings_scene)
    bpy.utils.register_class(DIESEL_PT_settings_mesh)
    bpy.types.TOPBAR_MT_file_export.append(menu_func_export)
    bpy.types.TOPBAR_MT_file_import.append(menu_func_import)

def unregister():
    bpy.types.TOPBAR_MT_file_import.remove(menu_func_import)
    bpy.types.TOPBAR_MT_file_export.remove(menu_func_export)
    bpy.utils.unregister_class(DIESEL_PT_settings_mesh)
    bpy.utils.unregister_class(DIESEL_PT_settings_scene)
    bpy.utils.unregister_class(DieselMeshSettings)
    bpy.utils.unregister_class(DieselSceneSettings)
    bpy.utils.unregister_class(ExportOilModel)
    bpy.utils.unregister_class(ImportDieselModel)
    bpy.utils.unregister_class(Pd2toolsPreferences)
