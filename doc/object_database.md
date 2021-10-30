# Diesel object databases
I'm not sure what the "real" name of these is but a class `ObjectDatabase` does
seem to be related inside of Diesel. Either way, models and shaders go in these
when compiled and bundled.

## Global definitions
Assume no padding anywhere here.

Column-major matrix:
```cpp
typedef Matrix4x4 float[16];
```

Vectors:
```cpp
typedef Vec3f float[3];
typedef Vec4f float[4];
typedef Rgba float[4];
```

Strings hashed with diesel's string-hashing function:
```cpp
typedef HashName uint64_t;
```

# Container format
```cpp
struct ObjectDatabase {
    uint32_t magic; // Either 0xFFFFFFFF or not present
    uint32_t section_count;
    uint32_t file_length;
    Section  sections[section_count]
};

struct Section {
    uint32_t type_id;
    uint32_t section_id;
    uint32_t length;
    uint8_t  data[length];
};
```
The interpretation of `Section::data` is indicated by the type ID, described in the following sections.

# Models (`.model`)
The bulk of ODB files are this. The following section IDs and names are known, but not all meanings are known.
| ID           | Name                            | Function                                     |
|--------------|---------------------------------|----------------------------------------------|
| `0x0ffcd100` | [Object3D](#Object3D)           | Empty object (GLTF calls this a Node)
| `0x33552583` | LightSet                        | ?
| `0x62212d88` | [Model](#Model)                 | Node with mesh data
| `0x7623c465` | [AuthorTag](#AuthorTag)         | Source file and author identifier
| `0x7ab072d3` | [Geometry](#Geometry)           | Vertex buffer
| `0x072b4d37` | SimpleTexture                   | ?
| `0x2c5d6201` | CubicTexture                    | ?
| `0x1d0b1808` | VolumetricTexture               | ?
| `0x3c54609c` | [Material](#Material)           | Name of material slot
| `0x29276b1d` | [MaterialGroup](#MaterialGroup) | Mapping of `Model`'s material IDs to `Material` sections.
| `0x2c1f096f` | NormalManagingGP                | ?
| `0x5ed2532f` | TextureSpaceGP                  | ?
| `0xe3a3b1ca` | [PassthroughGP](#PassthroughGP) | Reference to `Geometry` and `Topology`
| `0x65cc1825` | [SkinBones](#SkinBones)         | Mesh skinning data (bind transforms and mappings of joint numbers to the corresponding `Object3D`)
| `0x4c507a13` | [Topology](#Topology)           | Index buffer
| `0x03b634bd` | [TopologyIP](#TopologyIP)       | Reference to `Topology`
| `0x46bf31a7` | [Camera](#Camera)               | Node with camera
| `0xffa13b80` | [Light](#Light)                 | Node with light source
| `0x22126dc0` | LookAtRotationController        | ?
| `0x679d695b` | [LookAtConstrRotationController](#LookAtConstrRotationController) | ?
| `0x3d756e0c` | IKChainTarget                   | ?
| `0xf6c1eef7` | IKChainRotationController       | ?
| `0xdd41d329` | CompositeVector3Controller      | ?
| `0x95bb08f7` | CompositeRotationController     | ?
| `0x5dc011b8` | [AnimationData](#Animation-Controllers) | ? (Resembles keyframes with no actual data)
| `0x74f7363f` | Animatable                      | ?
| `0x186a8bbf` | KeyEvents                       | ?
| `0x7c7844fd` | [ModelToolHashes](#ModelToolHashes) | (Unofficial) List of unhashed names used in the file.

There are also keyframe data sections. Three are known for sure, but the others
have names that mostly strongly suggest what they are. The known ones all have [the same basic shape](#animation-controllers)
| ID           | Name                           | Known |
|--------------|--------------------------------|-------|
| `0x2060697e` | ConstFloatController           |       |
| `0x6da951b2` | StepFloatController            |       |
| `0x76bf5b66` | LinearFloatController          | Y     |
| `0x29743550` | BezierFloatController          |       |
| `0x5b0168d0` | ConstVector3Controller         |       |
| `0x544e238f` | StepVector3Controller          |       |
| `0x26a5128c` | LinearVector3Controller        | Y     |
| `0x28db639a` | BezierVector3Controller        |       |
| `0x33da0fc4` | XYZVector3Controller           |       |
| `0x2e540f3c` | ConstRotationController        |       |
| `0x033606e8` | EulerRotationController        |       |
| `0x007fb371` | QuatStepRotationController     |       |
| `0x648a206c` | QuatLinearRotationController   | Y     |
| `0x197345a5` | QuatBezRotationController      |       |

## Object3d
This is a non-abstract base class: the same layout turns up at the start of all
the things that have a position in 3D space. GLTF calls this a Node.

```cpp
struct Object3d {
    HashName  object_name;
    uint32_t  controller_count;
    uint32_t  animation_controller_ids[controller_count];
    Matrix4x4 transformation;
    Vec3f     translation;
    uint32_t  parent_id;
};
```
The translation coordinates overwrite the corresponding elements of the
transformation matrix.

The list of animation controllers can include zeroes, and is known to take on the following combinations:

| Object type    | Values provided by controller     | Meanings                       |
|----------------|-----------------------------------|--------------------------------|
| Light          | `float      null    null null`    | `intensity -      - -`         |
| Light          | `vector3    null    null`         | `colour    -      - -`         |
| Light          | `float      vector3 null vector3` | `intensity colour - position`  |
| Object3d/Model | `quaternion null    null`         | `rotation  -      -`           |
| Object3d/Model | `vector3`                         | Probably position              |
| Object3d/Model | `quaternion vector3`              | Probably rotation and position |

There are probably more, but even the obvious candidates have not yet been tested.

## Model
Subclass of Object3d. Brings things with bounding boxes into the world. Despite
the name, this is used both for actual models and for physics shapes that don't
have any geometry to them.

```cpp
struct Model : Object3d {
    ModelType type;
    union {
        Bounds bounds_only;
        Mesh   mesh;
    };
};

enum class ModelType : uint32_t {
    BoundsOnly = 6,
    Mesh = 3
};

// All of the bounds are in model space, the sphere is centered on model origin.
struct Bounds {
    Vec3f    bounding_box_min;
    Vec3f    bounding_box_max;
    float    bounding_sphere_radius;
    uint32_t unknown;
};

struct Mesh {
    uint32_t   geometry_provider_id; // Nonzero
    uint32_t   topology_ip_id;       // Nonzero
    uint32_t   renderatom_count;
    RenderAtom renderatoms[renderatom_count];
    Bounds     bounds;
    uint32_t   unknown_flags;
    uint32_t   skinbones_id; // Can be zero
};

// Unknown if actual meanings of unknown_flags bits
const uint32_t MESH_FLAG_SHADOWCASTER = 1;
const uint32_t MESH_FLAG_HASOPACITY = 2;
```

`RenderAtom`s select a single draw's worth of geometry. GLTF calls these Primitives. Getting the value wrong is... not crash-inducing, usually, but probably results in displaying nonsense. Some of the fields seem to be ignored by Diesel.
```cpp
struct RenderAtom {
    uint32_t base_vertex;    // Starting position in Geometry
    uint32_t triangle_count; // Number of triangles to draw
    uint32_t base_index;     // Index into Topology
    uint32_t vertex_count;   // Number of Geometry entries used
    uint32_t material_index; // Index into the mesh's MaterialGroup
}
```

## AuthorTag
Metadata, presumably for debugging purposes.
```cpp
struct AuthorTag {
    HashName unknown_1;          // Possibly "scene type"
    char     author_email[];     // Null terminated
    char     source_file_path[]; // Null terminated
    uint32_t unknown_2;
}
```

## Geometry
Vertex buffers. Unknown if attribute order is significant: there's no reason it should be and in any case in Payday 2's released models, the order is not consistent.

Geometry data is in struct-of-arrays form.
```cpp
struct Geometry {
    uint32_t        vertex_count;
    uint32_t        attribute_count;
    AttributeHeader attribute_info[attribute_count];
    Attribute       attributes[attribute_count];
    HashName        section_name
};

struct AttributeHeader {
    uint32_t format;
    uint32_t semantic;
};
```
Here `Attribute` isn't a real type, just denotes a dense array of `vertex_count` elements of the type according to the corresponding `AttributeHeader` and the following table:


| `semantic`  | Name | Type     | Notes                                                     |
|----|---------------|----------|-----------------------------------------------------------|
| 1  | Position      | float[3] |                                                           |
| 2  | Normal        | float[3] |                                                           |
| 3  | Position1     | float[3] |                                                           |
| 4  | Normal1       | float[3] |                                                           |
| 5  | Color0        | float[3] | Stored as BGRA                                            |
| 6  | Color1        | float[3] | Stored as BGRA                                            |
| 7  | TexCoord0     | float[2] |                                                           |
| 8  | TexCoord1     | float[2] |                                                           |
| 9  | TexCoord2     | float[2] |                                                           |
| 10 | TexCoord3     | float[2] |                                                           |
| 11 | TexCoord4     | float[2] |                                                           |
| 12 | TexCoord5     | float[2] |                                                           |
| 13 | TexCoord6     | float[2] |                                                           |
| 14 | TexCoord7     | float[2] |                                                           |
| 15 | BlendIndices0 | uint8[4] |                                                           |
| 16 | BlendIndices1 | uint8[4] |                                                           |
| 17 | BlendWeight0  | float[n] | `format` is number of components actually written (2/3/4) |
| 18 | BlendWeight1  | float[n] | `format` is number of components actually written (2/3/4) |
| 19 | PointSize     | float    |                                                           |
| 20 | Binormal      | float[3] |                                                           |
| 21 | Tangent       | float[3] |                                                           |

## Material
Names a material to select from a `.material_config` file.
```cpp
struct Material {
    HashName material_name;
    uint8_t  unknown_1[48];
    uint32_t item_count;
    Item     unknown_2[item_count];
};

struct Item {
    uint32_t unknown_1;
    uint32_t unknown_2;
};
```
## MaterialGroup
Maps the material indices in RenderAtoms to Materials
```cpp
struct MaterialGroup {
    uint32_t count;
    uint32_t material_ids[count];
};
```

## PassthroughGP
Refers to a Geometry and Topology.
```cpp
struct PassthroughGP {
    uint32_t geometry_id;
    uint32_t topology_id;
}
```
Unknown what the other \*GP types do, they're never used in Payday 2.

## SkinBones
This may as well be unknown for all the luck anyone's had generating it.

What is known is that it contains skinning data. It may be be a subclass of a `Bones` abstract class.

```cpp
struct SkinBones {
    uint32_t bm_count;
    BoneMapping bone_mappings[bm_count];

    uint32_t root_bone_id;

    uint32_t bone_count;
    uint32_t bone_node_ids[bone_count];
    Matrix4x4 bone_transform[bone_count];

    Matrix4x4 global_skin_transform;
    uint64_t  skin_name_hash;
}
```

Precise interpretation is unknown. Previous analysis says:

> Inside PD2, there is the `dsl::BoneMapping` class. This is used for some unknown purpose,
> however what is known is that it builds a list of matrices. These are referred to by
> indexes into a runtime table built by SkinBones (Bones::matrices).
> 
> This runtime table is built by multiplying together the world transform and global skin
> transform onto each SkinBones matrix. This is done in C#, loaded into the SkinPositions
> list in SkinBones.

`bm_count` seems to match the number of `RenderAtom`s in the Geometry.

### BoneMapping
What this struct does is unknown. Possibly a mapping from joint IDs in the Geometry to joints in the SkinBones.
```cpp
struct BoneMapping {
    uint32_t joint_id_count;
    uint32_t index_in_skinbones[joint_id_count];
}
```

## Topology
An index buffer.
```cpp
struct Topology {
    uint32_t unknown_1;

    uint32_t index_count;
    uint16_t indices[index_count];

    uint32_t ub_count;
    uint8_t unknown_2[ub_count];

    uint64_t section_name_hash;
}
```

## TopologyIP
Points to a Topology.
```cpp
struct Topology {
    topology: uint32_t // ID of Topology section
};
```

## Camera
Subclass of Object3D, representing some sort of camera. Details entirely unknown.
```cpp
struct Camera : Object3d {
    float unknown_1;
    float unknown_2;
    float unknown_3;
    float unknown_4;
    float unknown_5;
    float unknown_6;
};
```

## Light
Node that's a light source.
```cpp
struct Light : Object3d {
    uint8_t unknown_1;
    LightType light_type;
    float[4] color;
    float near_range;
    float far_range;
    float unknown_6;
    float unknown_7;
    float unknown_8;
};

enum class LightType : uint32_t {
    Omnidirectional = 1,
    Spot = 2
};
```
## LookAtConstrRotationController
Presumably this points nodes at each other, but actual functioning unknown.

In the two files it's used, it is indeed used like a rotation.
```cpp
struct LookAtConstrRotationController {
    HashName name;
    uint32_t unknown;
    uint32_t object3d_id_1; 
    uint32_t object3d_id_2; 
    uint32_t object3d_id_3; 
}
```

## AnimationData
Unknown purpose, but shows up regularly.
```cpp
struct AnimationController {
    HashName    name;
    uint32_t    unknown_2;
    float       duration;
    uint32_t    kf_count;
    Keyframe<T> keyframes[kf_count];
}
```

## Animation controllers
There are a bunch of these. The known ones all have the same shape.
```cpp
template<typename T>
struct Keyframe<T> {
    float time;
    T     data;
};

template<typename T>
struct AnimationController {
    HashName    controller_name;
    uint32_t    unknown_flags;
    uint32_t    unknown_2;
    float       duration;
    uint32_t    kf_count;
    Keyframe<T> keyframes[kf_count];
};

typedef LinearVector3Controller      AnimationController<Vec3f>;
typedef LinearFloatController        AnimationController<float>;
typedef QuatLinearRotationController AnimationController<Vec4f>;
```

## ModelToolHashes
This is an entirely unofficial one, the community-made tool for converting to/from .model emits this to record names that were hashed in the process of generating a .model file.
```cpp
struct ModelToolHashes {
    uint16_t version;
    uint32_t string_count;
    Utf8String strings[string_count];
};

struct Utf8String {
    uint16_t length;
    uint8_t data[length]; // UTF-8 string
}
```

# Shaders
I don't know the full details here, but there's three sections that seem to be relevant:
| ID           | Name                           |
|--------------|--------------------------------|
| `0x7f3552d1` | D3DShader                      |
| `0x214b1aaf` | D3DShaderPass                  |
| `0x12812c1a` | D3DShaderLibrary               |