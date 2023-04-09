use pyo3::types::PyDict;
use pyo3::{prelude::*, intern, AsPyPointer};
type Vec2f = vek::Vec2<f32>;
type Vec3f = vek::Vec3<f32>;
type Vec4f = vek::Vec4<f32>;
type Transform = vek::Transform<f32, f32, f32>;
type Quaternion = vek::Quaternion<f32>;

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

macro_rules! bpy_struct_wrapper {
    ($name:ident) => {
        #[derive(Copy,Clone)]
        pub struct $name<'py>(&'py PyAny);
        impl<'py> $name<'py> {
            pub fn new(r: &'py PyAny) -> Self {
                Self(r)
            }
        
            pub fn as_ptr(&self) -> *mut pyo3::ffi::PyObject { self.0.as_ptr() }
        }
        impl<'py> FromPyObject<'py> for $name<'py> {
            fn extract(ob: &'py PyAny) -> PyResult<Self> {
                Ok(Self::new(ob))
            }
        }
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
    for c in 0..4 {
        let col = bmat.get_item(c).unwrap();
        for r in 0..4 {
            let cell = col.get_item(r).unwrap().extract::<f32>().unwrap();
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

/// Blender Object
#[derive(Clone, Copy)]
pub struct Object<'py>(&'py PyAny);
impl<'py> Object<'py> {
    pub fn new(r: &'py PyAny) -> Object<'py> {
        Self(r)
    }

    pub fn as_ptr(&self) -> *mut pyo3::ffi::PyObject { self.0.as_ptr() }

    attr_get!(name: "name" => &str );
    attr_get!(r#type: "type" => ObjectType );
    attr_get!(parent: "parent" => Option<Object>);
    attr_get!(parent_type: "parent_type" => ParentType);
    attr_get!(matrix_local: "matrix_local" => Transform as transform_from_bpy_matrix);
    attr_get!(parent_bone: "parent_bone" => &str);
    attr_get!(matrix_world: "matrix_world" => vek::Mat4<f32> as mat4_from_bpy_matrix);
    attr_get!(data: "data" => &PyAny);

    iter_get!(iter_modifiers: "modifiers");
    iter_get!(iter_vertex_groups: "vertex_groups");
    iter_get!(iter_material_slots: "material_slots");
    iter_get!(iter_children_recursive: "children_recursive" => Object);

    method!(evaluated_get: "evaluated_get"(depsgraph: &'py PyAny) -> Object<'py>);
    pub fn to_mesh(&self, preserve_all_data_layers: bool, depsgraph: &'py PyAny) -> PyResult<&'py PyAny> {
        let args = PyDict::new(self.0.py());
        args.set_item("preserve_all_data_layers", preserve_all_data_layers).unwrap();
        args.set_item("depsgraph", depsgraph).unwrap();
        self.0.call_method(intern!(self.0.py(), "to_mesh"), (), Some(args))
    }
    method!(to_mesh_clear: "to_mesh_clear"());
}
impl<'py> FromPyObject<'py> for Object<'py> {
    fn extract(ob: &'py PyAny) -> PyResult<Self> {
        Ok(Self::new(ob))
    }
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

pub struct Bone<'py>(&'py PyAny);
impl<'py> Bone<'py> {
    pub fn new(r: &'py PyAny) -> Self {
        Self(r)
    }
    attr_get!(name: "name" => &str );
    attr_get!(head: "head" => Vec3f as vek3f_from_bpy_vec);
    attr_get!(tail: "tail" => Vec3f as vek3f_from_bpy_vec);
    attr_get!(parent: "parent" => Option<Bone>);
    attr_get!(matrix_local: "matrix_local" => vek::Mat4<f32> as mat4_from_bpy_matrix);
    attr_get!(matrix: "matrix" => &PyAny);
}
impl<'py> FromPyObject<'py> for Bone<'py> {
    fn extract(ob: &'py PyAny) -> PyResult<Self> {
        Ok(Self::new(ob))
    }
}

bpy_struct_wrapper!(Armature);
impl<'py> Armature<'py> {
    iter_get!(iter_bones: "bones" => Bone);
}