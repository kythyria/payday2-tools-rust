use super::file;
/*mod v0 {
    use std::collections::HashMap;

    use super::file;

    struct Document {
        scene_info: file::SceneInfo3,
        material_xml: String,
        root_object: Option<Box<Object>>,
        materials: HashMap<u32, Material>
    }

    struct Object {
        node: file::Node,
        data: ObjectData,
        
        anim_position: Option<Box<Animation<vek::Vec3<f64>>>>,
        anim_rotation: Option<Box<Animation<vek::Quaternion<f64>>>>,
        anim_composite_position: Option<Box<CompositeAnimation<vek::Vec3<f64>>>>,
        anim_composite_rotation: Option<Box<CompositeAnimation<vek::Quaternion<f64>>>>,
        anim_lookat: Option<Box<LookatController>>,
        anim_ik_target: Option<Box<IkTargetController>>,
        anim_ik_chain: Option<Box<IkChainController>>,

        children: Vec<Box<Object>>
    }

    enum ObjectData {
        Empty,
        Geometry(Box<Geometry)>,
        Light(Box<Light)>,
        Camera(Box<Camera>)
    }

    struct Light {
        data: file::Light,

        anim_color: Option<Box<Animation<vek::Rgb<f64>>>>,
        anim_attenuation: Option<Box<Animation<f64>>>,
        anim_multiplier: Option<Box<Animation<f64>>>,
        anim_hotspot: Option<Box<Animation<f64>>>,
        anim_falloff: Option<Box<Animation<f64>>>,
    }

    struct Camera {
        data: file::Camera
    }
}
*/

mod v1 {
    use super::file;

    slotmap::new_key_type! {
        struct ObjectKey;
        struct MaterialKey;
    }

    struct Document {
        scene_info: file::SceneInfo3,
        material_xml: String,
        objects: slotmap::SlotMap<ObjectKey, ObjectInternal>,
        //cameras: slotmap::SparseSecondaryMap<ObjectKey, CameraInternal>,
        //lights: slotmap::SparseSecondaryMap<ObjectKey, LightInternal>,
        //meshes: slotmap::SparseSecondaryMap<ObjectKey, MeshInternal>
        root_object: Option<ObjectKey>
        //materials: slotmap::SlotMap<MaterialKey, MaterialInternal>,
        //key_events: KeyEventsInternal
    }
    impl Document {
        fn get_object(&self, key: ObjectKey) -> Option<Object> {
            if self.objects.contains_key(key) {
                Some(Object(self, key))
            }
            else {
                None
            }
        }

        fn get_object_mut(&self, key: ObjectKey) -> Option<Object> {
            if self.objects.contains_key(key) {
                Some(Object(self, key))
            }
            else {
                None
            }
        }
    }

    struct ObjectInternal {
        parent: Option<ObjectKey>,
        first_child: Option<ObjectKey>,
        next_sibling: Option<ObjectKey>,

        name: String,
        transform: vek::Mat4<f64>,
        pivot_transform: vek::Mat4<f64>,

        //anim_position: Option<Box<Animation<vek::Vec3<f64>>>>,
        //anim_rotation: Option<Box<Animation<vek::Quaternion<f64>>>>,
        //anim_composite_position: Option<Box<CompositeAnimation<vek::Vec3<f64>>>>,
        //anim_composite_rotation: Option<Box<CompositeAnimation<vek::Quaternion<f64>>>>,
        //anim_lookat: Option<Box<LookatControllerInternal>>,
        //anim_ik_target: Option<Box<IkTargetControllerInternal>>,
        //anim_ik_chain: Option<Box<IkChainControllerInternal>>,
    }

    struct Object<'a>(&'a Document, ObjectKey);
    impl<'a> Object<'a> {
        fn parent(&self) -> Option<Object> {
            let obj = self.0.objects.get(self.1).unwrap();
            obj.parent.and_then(|i| self.0.get_object(i))
        }

        fn first_child(&self) -> Option<Object> {
            let obj = self.0.objects.get(self.1).unwrap();
            obj.first_child.and_then(|i| self.0.get_object(i))
        }

        fn next_sibling(&self) -> Option<Object> {
            let obj = self.0.objects.get(self.1).unwrap();
            obj.next_sibling.and_then(|i| self.0.get_object(i))
        }

        fn name(&self) -> &str {
            let obj = self.0.objects.get(self.1).unwrap();
            obj.name.as_str()
        }

        fn transform(&self) -> &vek::Mat4<f64> {
            let obj = self.0.objects.get(self.1).unwrap();
            &obj.transform
        }

        fn pivot_transform(&self) -> &vek::Mat4<f64> {
            let obj = self.0.objects.get(self.1).unwrap();
            &obj.pivot_transform
        }
    }

    impl<'a> Object<'a> {
        fn parent_mut(&mut self) -> Option<Object> {
            let obj = self.0.objects.get(self.1).unwrap();
            obj.parent.and_then(|i| self.0.get_object_mut(i))
        }

        fn first_child_mut(&mut self) -> Option<Object> {
            let obj = self.0.objects.get(self.1).unwrap();
            obj.first_child.and_then(|i| self.0.get_object_mut(i))
        }

        fn next_sibling_mut(&mut self) -> Option<Object> {
            let obj = self.0.objects.get(self.1).unwrap();
            obj.next_sibling.and_then(|i| self.0.get_object_mut(i))
        }

        fn name_mut(&mut self) -> &mut String {
            let obj = self.0.objects.get(self.1).unwrap();
            &mut obj.name
        }

        fn transform_mut(&mut self) -> &vek::Mat4<f64> {
            let obj = self.0.objects.get(self.1).unwrap();
            &mut obj.transform
        }

        fn pivot_transform_mut(&mut self) -> &vek::Mat4<f64> {
            let obj = self.0.objects.get(self.1).unwrap();
            &mut obj.pivot_transform
        }
    }

    struct CameraInternal {
        fov: f64,
        far_clip: f64,
        near_clip: f64,
        target_key: ObjectKey,
        target_distance: f64,
        aspect_ratio: f64,

        //anim_fov: Option<Box<Animation<f64>>>,
        //anim_far_clip: Option<Box<Animation<f64>>>,
        //anim_near_clip: Option<Box<Animation<f64>>>,
        //anim_target_distance: Option<Box<Animation<f64>>>,
    }
}