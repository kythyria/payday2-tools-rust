use std::marker::PhantomData;

use pd2tools_macros::WrapsPyAny;
use pyo3::types::PyDict;
use pyo3::{prelude::*, intern, AsPyPointer};
use crate::vek_types::*;

macro_rules! get {
    ($ob:expr, 'attr $field:literal) => {
        $ob.getattr(intern!{$ob.py(), $field}).unwrap().extract().unwrap()
    };
    ($ob:expr, 'iter $field:literal) => {
        $ob.getattr(intern!{$ob.py(), $field})
            .unwrap()
            .iter()
            .unwrap()
            .map(Result::unwrap)
    };
}

pub trait WrapsPyAny<'py> {
    fn py(&self) -> Python<'py>;
    fn as_ptr(&self) -> *mut pyo3::ffi::PyObject;
    fn as_pyany(&self) -> &'py PyAny;
}

macro_rules! bpy_struct_wrapper {
    ($name:ident) => {
        #[derive(Copy,Clone,WrapsPyAny)]
        pub struct $name<'py>(&'py PyAny);
        //impl<'py> std::ops::Deref for $name<'py> {
        //    type Target = PyAny;
        //
        //    fn deref(&self) -> &Self::Target {
        //        self.0
        //    }
        //}
    }
}

macro_rules! attr_get {
    ($getter:ident: $py_name:expr => $type:ty) => {
        pub fn $getter(&self) -> $type {
            self.0.getattr(intern!{self.0.py(), $py_name}).unwrap().extract().unwrap()
        }
    };
    ($getter:ident: $py_name:expr => $type:ty as $converter:path) => {
        pub fn $getter(&self) -> $type {
            let v: &PyAny = self.0.getattr(intern!{self.0.py(), $py_name}).unwrap();
            $converter(v)
        }
    };
}

macro_rules! iter_get {
    ($getter: ident: $py_name: expr) => {
        pub fn $getter(&self) -> impl Iterator<Item=&PyAny> {
            self.0.getattr(intern!{self.0.py(), $py_name})
            .unwrap()
            .iter()
            .unwrap()
            .map(Result::unwrap)
        }
    };
    ($getter: ident: $py_name: expr => $type:ty) => {
        pub fn $getter(&self) -> impl Iterator<Item=$type> {
            self.0.getattr(intern!{self.0.py(), $py_name})
            .unwrap()
            .iter()
            .unwrap()
            .map(Result::unwrap)
            .map(FromPyObject::extract)
            .map(Result::unwrap)
        }
    };
}

macro_rules! method {
    ($name:ident: $py_name:literal()) => {
        pub fn $name(&self) {
            self.0.call_method0(intern!(self.0.py(), $py_name)).unwrap();
        }
    };

    ($name:ident: $py_name:literal() -> $type:ty $(as $converter:path)?) => {
        pub fn $name(&self) -> $type {
            self.0.call_method0(intern!(self.0.py(), $py_name)).unwrap().extract().unwrap()
        }
    };
    
    ($name:ident: $py_name:literal($($arg:ident: $arg_ty:ty),*) -> $type:ty $(as $converter:path)?) => {
        pub fn $name(&self $(,$arg: $arg_ty)*) -> $type {
            self.0.call_method1(intern!(self.0.py(), $py_name), ($($arg,)*)).unwrap().extract().unwrap()
        }
    }
}

macro_rules! bpy_str_enum {
    ($v:vis enum $name:ident {
        $($variant:ident = $pystr:literal),* $(,)?
    }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        $v enum $name {
            $($variant),*
        }
        impl<'py> FromPyObject<'py> for $name {
            fn extract(ob: &'py PyAny) -> PyResult<Self> {
                let s: &str = ob.extract()?;
                match s {
                    $($pystr => Ok(Self::$variant),)*
                    _ => Err(pyo3::PyDowncastError::new(ob, std::any::type_name::<Self>()).into())
                }
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
                match self {
                    $(Self::$variant => write!(f, "{:?}", $pystr),)*
                }
            }
        }
    }
}

pub trait PropCollection: IntoIterator {
    fn len(&self) -> usize;
    fn iter(&self) -> TypedPyIterator<Self::Item>;
}
pub trait DictPropCollection: PropCollection {
    fn get_key(&self, key: &str) -> Option<Self::Item>;
}
pub trait ArrayPropCollection: PropCollection  {
    fn get_idx(&self, key: usize) -> Option<Self::Item>;
}

/// Types that wrap an array we could use directly
/// Unsafe because to implement this, `as_data_pointer` and `len` have to add up to a valid slice.
/// Since they come from outside, the compiler can't check that.
pub unsafe trait PodArray {
    type Item: bytemuck::Pod;
    
    fn as_data_pointer(&self) -> *const Self::Item;
    fn len(&self) -> usize;
    fn as_slice(&self) -> &[Self::Item];
    fn to_vec(&self) -> Vec<Self::Item> {
        Vec::from(self.as_slice())
    }
}

macro_rules! bpy_collection {
    ($name:ident, 'array $item:ty) => {
        bpy_struct_wrapper!($name);
        impl<'py> IntoIterator for $name<'py> {
            type Item = $item;
            type IntoIter = TypedPyIterator<'py, $item>;
            fn into_iter(self) -> Self::IntoIter {
                TypedPyIterator(self.0.iter().unwrap(), PhantomData)
            }
        }
        impl<'py> PropCollection for $name<'py> {
            fn len(&self) -> usize { self.0.len().unwrap() }
            fn iter(&self) -> TypedPyIterator<Self::Item> {
                TypedPyIterator(self.0.iter().unwrap(), PhantomData)
            }
        }
        impl<'py> ArrayPropCollection for $name<'py> {
            fn get_idx(&self, key: usize) -> Option<Self::Item> {
                let it = self.0.get_item(key);
                match it {
                    Err(e) if e.is_instance_of::<pyo3::exceptions::PyIndexError>(self.py()) => { None },
                    it => Some(it.unwrap().extract().unwrap())
                }
            }
        }
    };
    ($name: ident, 'arraydict $item:ty) => {
        bpy_collection!($name, 'array $item);
        impl<'py> DictPropCollection for $name<'py> {
            // TODO: Is this prone to stray copies?
            fn get_key(&self, key: &str) -> Option<Self::Item> {
                let it = self.0.get_item(key);
                match it {
                    Err(e) if e.is_instance_of::<pyo3::exceptions::PyIndexError>(self.py()) => { None },
                    it => Some(it.unwrap().extract().unwrap())
                }
            }
        }
    }
}

//#[derive(Copy,Clone)]
//struct BpyCollection<'py, T>(&'py PyAny, PhantomData<T>);
#[derive(Copy,Clone,WrapsPyAny)]
pub struct BpyCollection<'py, T>(&'py PyAny, PhantomData<T>);
impl<'py,T: FromPyObject<'py>+Clone> IntoIterator for BpyCollection<'py,T>{
  type Item = T;
  type IntoIter = TypedPyIterator<'py, T>;
  fn into_iter(self) -> Self::IntoIter {
    TypedPyIterator(self.0.iter().unwrap(),PhantomData)
  }
}
impl<'py,T: FromPyObject<'py>+Clone> PropCollection for BpyCollection<'py,T> {
  fn len(&self) -> usize { self.0.len().unwrap() }
  fn iter(&self) -> TypedPyIterator<Self::Item> {
    TypedPyIterator(self.0.iter().unwrap(),PhantomData)
  }
}

pub struct TypedPyIterator<'py, T>(&'py pyo3::types::PyIterator, PhantomData<T>);
impl<'py, T> std::ops::Deref for TypedPyIterator<'py, T> {
    type Target = &'py pyo3::types::PyIterator;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'py, T: FromPyObject<'py>> Iterator for TypedPyIterator<'py, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|i| i.unwrap().extract().unwrap())
    }
}

fn vek2f_from_tuple(inp: (f32, f32)) -> Vec2f {
    inp.into()
}

fn vek3f_from_tuple(inp: (f32, f32, f32)) -> Vec3f {
    inp.into()
}

fn vek2f_from_bpy_vec(data: &PyAny) -> Vec2f {
    let tuple = data.call_method0(intern!(data.py(), "to_tuple")).unwrap().extract().unwrap();
    vek2f_from_tuple(tuple)
}

fn vek3f_from_bpy_vec(data: &PyAny) -> Vec3f {
    let tuple = data.call_method0(intern!(data.py(), "to_tuple")).unwrap().extract().unwrap();
    vek3f_from_tuple(tuple)
}

fn quaternion_from_bpy_quat(bq: &PyAny) -> Quaternion {
    let x: f32 = get!(bq, 'attr "x");
    let y: f32 = get!(bq, 'attr "y");
    let z: f32 = get!(bq, 'attr "z");
    let w: f32 = get!(bq, 'attr "w");
    Quaternion::from_xyzw(x, y, z, w)
}

fn mat4_from_bpy_matrix(bmat: &PyAny) -> vek::Mat4<f32> {
    let mut floats = [[0f32; 4]; 4];
    for r in 0..4 {
        let row = bmat.get_item(r).unwrap();
        for c in 0..4 {
            let cell = row.get_item(c).unwrap().extract::<f32>().unwrap();
            floats[c][r] = cell;
        }
    }
    vek::Mat4::from_col_arrays(floats)
}

fn transform_from_bpy_matrix(bmat: &PyAny) -> Transform {
    let py_lrs = bmat.call_method0(intern!{bmat.py(), "decompose"}).unwrap();
    let (py_loc, py_rot, py_scale): (&PyAny, &PyAny, &PyAny) = py_lrs.extract().unwrap();
    Transform {
        position: vek3f_from_bpy_vec(py_loc),
        orientation: quaternion_from_bpy_quat(py_rot),
        scale: vek3f_from_bpy_vec(py_scale)
    }
}

fn from_bpy_array<const N:usize,T,E>(data: &PyAny) -> T
where
    T: From<[E; N]>,
    E: Default + Copy + for<'a> FromPyObject<'a>
{
    let mut a: [E; N] = [E::default(); N];
    for i in 0..N {
        a[i] = data.get_item(i).unwrap().extract().unwrap();
    }
    T::from(a)
}

/// Blender Object
#[derive(Clone, Copy, WrapsPyAny)]
pub struct Object<'py>(&'py PyAny);
impl<'py> Object<'py> {
    attr_get!(name: "name" => &str );
    attr_get!(r#type: "type" => ObjectType );
    attr_get!(parent: "parent" => Option<Object>);
    attr_get!(parent_type: "parent_type" => ParentType);
    attr_get!(matrix_local: "matrix_local" => Transform as transform_from_bpy_matrix);
    attr_get!(parent_bone: "parent_bone" => &str);
    attr_get!(matrix_world: "matrix_world" => vek::Mat4<f32> as mat4_from_bpy_matrix);
    attr_get!(data: "data" => &PyAny);

    iter_get!(iter_modifiers: "modifiers" => Modifier<'py>);
    iter_get!(iter_vertex_groups: "vertex_groups" => VertexGroup);
    iter_get!(iter_material_slots: "material_slots" => MaterialSlot);
    iter_get!(iter_children_recursive: "children_recursive" => Object);

    method!(evaluated_get: "evaluated_get"(depsgraph: &'py PyAny) -> Object<'py>);
    pub fn to_mesh(&self, preserve_all_data_layers: bool, depsgraph: &'py PyAny) -> Mesh<'py> {
        let args = PyDict::new(self.0.py());
        args.set_item("preserve_all_data_layers", preserve_all_data_layers).unwrap();
        args.set_item("depsgraph", depsgraph).unwrap();
        let d = self.0.call_method(intern!(self.0.py(), "to_mesh"), (), Some(args)).unwrap();
        Mesh::wrap(d)
    }
    method!(to_mesh_clear: "to_mesh_clear"());
}

bpy_str_enum!{
    pub enum ObjectType {
        Mesh = "MESH",
        Curve = "CURVE",
        Surface = "SURFACE",
        Metaball = "META",
        Text = "FONT",
        HairCurves = "CURVES",
        PointCloud = "POINTCLOUD",
        Volume = "VOLUME",
        GreasePencil = "GPENCIL",
        Armature = "ARMATURE",
        Lattice = "LATTICE",
        Empty = "EMPTY",
        Light = "LIGHT",
        LightProbe = "LIGHT_PROBE",
        Camera = "CAMERA",
        Speaker = "SPEAKER"
    }
}

bpy_str_enum!{
    pub enum ParentType {
        Object = "OBJECT",
        Armature = "ARMATURE",
        Lattice = "LATTICE",
        Vertex = "VERTEX",
        Vertex3 = "VERTEX_3",
        Bone = "BONE",
    }
}

bpy_struct_wrapper!(VertexGroup);
impl<'py> VertexGroup<'py> {
    attr_get!(name: "name" => &'py str);
}

bpy_struct_wrapper!(MaterialSlot);
impl<'py> MaterialSlot<'py> {
    attr_get!(material: "material" => Option<Material<'py>>);
}

bpy_struct_wrapper!(Material);
impl<'py> Material<'py> {
    attr_get!(name: "name" => &'py str);
}

pub struct Bone<'py>(&'py PyAny);
impl<'py> Bone<'py> {
    pub fn wrap(r: &'py PyAny) -> Self {
        Self(r)
    }
    attr_get!(name: "name" => &str );
    attr_get!(head: "head" => Vec3f as vek3f_from_bpy_vec);
    attr_get!(tail: "tail" => Vec3f as vek3f_from_bpy_vec);
    attr_get!(parent: "parent" => Option<Bone>);
    attr_get!(matrix_local: "matrix_local" => vek::Mat4<f32> as mat4_from_bpy_matrix);
    attr_get!(matrix: "matrix" => BMathMatrix);
    attr_get!(length: "length" => f32);
}
impl<'py> FromPyObject<'py> for Bone<'py> {
    fn extract(ob: &'py PyAny) -> PyResult<Self> {
        Ok(Self::wrap(ob))
    }
}

bpy_struct_wrapper!(Armature);
impl<'py> Armature<'py> {
    attr_get!(bones: "bones" => ArmatureBones<'py>);

    iter_get!(iter_bones: "bones" => Bone);
}
bpy_collection!(ArmatureBones, 'arraydict Bone<'py>);

bpy_struct_wrapper!(Mesh);
impl<'py> Mesh<'py> {
    pub fn calc_tangents(&self) -> PyResult<()> {
        self.0.call_method0(intern!{self.py(), "calc_tangents"}).map(|_|())
    }
    attr_get!(vertices: "vertices" => MeshVertices);
    attr_get!(loops: "loops" => MeshLoops);
    attr_get!(polygons: "polygons" => MeshPolygons);
    attr_get!(loop_triangles: "loop_triangles" => MeshLoopTriangles);
    attr_get!(attributes: "attributes" => AttributeGroup);
    attr_get!(uv_layers: "uv_layers" => UvLoopLayers);
    attr_get!(diesel_settings: "diesel" => &'py PyAny);
    iter_get!(iter_vertices: "vertices" => MeshVertex);

    method!(calc_loop_triangles: "calc_loop_triangles"());
    method!(calc_normals_split: "calc_normals_split"());
}

bpy_collection!(MeshVertices, 'array MeshVertex<'py>);
bpy_collection!(MeshVertexGroups, 'array VertexGroupElement<'py>);
bpy_collection!(MeshLoops, 'array MeshLoop<'py>);
bpy_collection!(MeshPolygons, 'array MeshPolygon<'py>);
bpy_collection!(MeshLoopTriangles, 'array MeshLoopTriangle<'py>);
bpy_collection!(AttributeGroup, 'array Attribute<'py>);
bpy_collection!(UvLoopLayers, 'array MeshUvLoopLayer<'py>);

bpy_struct_wrapper!(MeshVertex);
impl<'py> MeshVertex<'py> {
    attr_get!(co: "co" => Vec3f as vek3f_from_bpy_vec);
    attr_get!(groups: "groups" => MeshVertexGroups);
}

bpy_struct_wrapper!(VertexGroupElement);
impl<'py> VertexGroupElement<'py> {
    attr_get!(group: "group" => usize);
    attr_get!(weight: "weight" => f32);
}

bpy_struct_wrapper!(MeshLoop);
impl<'py> MeshLoop<'py> {
    attr_get!(vertex_index: "vertex_index" => usize);
    attr_get!(edge_index: "edge_index" => usize);
    attr_get!(normal: "normal" => Vec3f as vek3f_from_bpy_vec);
    attr_get!(tangent: "tangent" => Vec3f as vek3f_from_bpy_vec);
    attr_get!(bitangent: "bitangent" => Vec3f as vek3f_from_bpy_vec);

}

bpy_struct_wrapper!(MeshPolygon);
impl<'py> MeshPolygon<'py> {
    attr_get!(loop_start: "loop_start" => usize);
    attr_get!(loop_total: "loop_total" => usize);
    attr_get!(material_index: "material_index" => usize);
}

bpy_struct_wrapper!(MeshLoopTriangle);
impl<'py> MeshLoopTriangle<'py> {
    attr_get!(loops: "loops" => [usize; 3] as from_bpy_array);
    attr_get!(polygon_index: "polygon_index" => usize);
}

bpy_struct_wrapper!(Attribute);
impl<'py> Attribute<'py> {
    attr_get!(name: "name" => &str);
    attr_get!(domain: "domain" => AttributeDomain);
    attr_get!(data_type: "data_type" => AttributeType);

    // TODO: Make this actually typesafe
    attr_get!(bool_data: "data" => BpyCollection<AttributeScalarValue<bool>>);
    attr_get!(i8_data: "data" => BpyCollection<AttributeScalarValue<u8>>);
    attr_get!(vec2f_data: "data" => BpyCollection<AttributeVek2fValue>);
    attr_get!(f32_data: "data" => BpyCollection<AttributeScalarValue<f32>>);
    attr_get!(vec3f_data: "data" => BpyCollection<AttributeVek3fValue>);
    attr_get!(i32_data: "data" => BpyCollection<AttributeScalarValue<i32>>);
    attr_get!(str_data: "data" => BpyCollection<AttributeScalarValue<&str>>);
    attr_get!(f32_color_data: "data" => BpyCollection<AttributeColorValue>);
    attr_get!(u8_color_data: "data" => BpyCollection<AttributeColorValue>);
}

#[derive(Copy, Clone, WrapsPyAny)]
pub struct AttributeScalarValue<'py,T>(&'py PyAny, PhantomData<T>);
impl<'py, T: FromPyObject<'py>> AttributeScalarValue<'py, T> {
    attr_get!(value: "value" => T);
}

#[derive(Copy, Clone, WrapsPyAny)]
pub struct AttributeColorValue<'py>(&'py PyAny);
impl<'py> AttributeColorValue<'py> {
    attr_get!(value: "color" => Rgbaf as from_bpy_array);
}

#[derive(Copy, Clone, WrapsPyAny)]
pub struct AttributeVek2fValue<'py>(&'py PyAny);
impl<'py> AttributeVek2fValue<'py> {
    attr_get!(value: "vector" => Vec2f as vek2f_from_bpy_vec);
}

#[derive(Copy, Clone, WrapsPyAny)]
pub struct AttributeVek3fValue<'py>(&'py PyAny);
impl<'py> AttributeVek3fValue<'py> {
    attr_get!(value: "vector" => Vec3f as vek3f_from_bpy_vec);
}

bpy_str_enum!{
    pub enum AttributeDomain {
        Point = "POINT",
        Edge = "EDGE",
        Face = "FACE",
        Faceloop = "CORNER",
        Spline = "CURVE",
        Instance = "INSTANCE",
    }
}
bpy_str_enum! {
    pub enum AttributeType {
        F32 = "FLOAT",
        I32 = "INT",
        Vec3f = "FLOAT_VECTOR",
        FloatColor = "FLOAT_COLOR",
        ByteColor = "BYTE_COLOR",
        String = "STRING",
        Bool = "BOOLEAN",
        Vec2f = "FLOAT2",
        I8 = "INT8",
    }
}

#[derive(Copy, Clone, WrapsPyAny)]
pub struct MeshUvLoopLayer<'py>(&'py PyAny);
impl<'py> MeshUvLoopLayer<'py> {
    attr_get!(name: "name" => &str);
    attr_get!(uv: "uv" => BpyCollection<AttributeVek2fValue>);
}

bpy_struct_wrapper!(Modifier);
impl<'py> Modifier<'py> {
    attr_get!(show_viewport: "show_viewport" => bool);
    
    pub fn set_show_viewport(&self, vis: bool) {
        self.as_pyany().setattr(intern!(self.py(), "show_viewport"), vis).unwrap()
    }

    pub fn try_into_armature(self) -> Option<ArmatureModifier<'py>> {
        let typ: &str = get!(self.as_pyany(), 'attr "type");
        if typ == "ARMATURE" {
            Some(ArmatureModifier::wrap(self.as_pyany()))
        }
        else {
            None
        }
    }
}

bpy_struct_wrapper!(ArmatureModifier);
impl<'py> ArmatureModifier<'py> {
    attr_get!(show_viewport: "show_viewport" => bool);
    
    pub fn set_show_viewport(&self, vis: bool) {
        self.as_pyany().setattr(intern!(self.py(), "show_viewport"), vis).unwrap()
    }

    attr_get!(object: "object" => Object);
}



pub mod bmesh {
    use pyo3::{intern, prelude::*};

    pub fn new<'py>(py: Python<'py>) -> PyResult<BMesh<'py>> {
        BMesh::new(py)
    }

    pub struct BMesh<'py>(&'py PyAny, Python<'py>);
    impl Drop for BMesh<'_> {
        fn drop(&mut self) {
            match self.0.call_method0(intern!{self.1, "free"}) {
                _ => ()
            }
        }
    }
    impl<'py> BMesh<'py> {
        pub fn new(py: Python<'py>) -> PyResult<BMesh<'py>> {
            py.import("bmesh")
                .unwrap()
                .call_method0(intern!{py, "new"})
                .map(|bm| BMesh(bm, py))
        }
        pub fn free(self) { }
        pub fn from_mesh(&self, mesh: &'py PyAny) -> PyResult<()> {
            self.0.call_method1(intern!{self.1, "from_mesh"}, (mesh,))
                .map(|_|())
        }
        pub fn faces(&self) -> PyResult<&'py PyAny> {
            self.0.getattr(intern!{self.1, "faces"})
        }
        pub fn to_mesh(&self, mesh: &'py PyAny) -> PyResult<()> {
            self.0.call_method1(intern!{self.1, "to_mesh"}, (mesh,))
                .map(|_|())
        }
    }
    impl IntoPy<pyo3::Py<pyo3::PyAny>> for BMesh<'_> {
        fn into_py(self, _py: Python<'_>) -> pyo3::Py<pyo3::PyAny> {
            self.0.into()
        }
    }
    impl IntoPy<pyo3::Py<pyo3::PyAny>> for &BMesh<'_> {
        fn into_py(self, _py: Python<'_>) -> pyo3::Py<pyo3::PyAny> {
            self.0.into()
        }
    }

    #[derive(Clone, Copy)]
    pub struct Ops<'py>(&'py PyModule, Python<'py>);
    impl<'py> Ops<'py> {
        pub fn import(py: Python<'py>) -> Self {
            Self(py.import("bmesh.ops").unwrap(), py)
        }
        pub fn triangulate(&self, mesh: &'py BMesh<'py>, faces: &'py PyAny) -> PyResult<&PyAny> {
            let args = pyo3::types::PyDict::new(self.1);
            args.set_item("faces", faces).unwrap();
            self.0.call_method(intern!{self.1, "triangulate"}, (mesh,), Some(args))
        }
    }
}

bpy_struct_wrapper!(BMathMatrix);
impl<'py> BMathMatrix<'py> {
    pub fn to_quaternion(&self) -> Quaternion {
        let quat = self.as_pyany().call_method0(intern!{self.py(), "to_quaternion"}).unwrap();
        quaternion_from_bpy_quat(quat)
    }
}