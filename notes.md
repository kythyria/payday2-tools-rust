The order of sections in FDM seems to be
```
<FDM> = AnimationData AuthorTag <Materials> <Node>+ <Mesh>* <Animation>*
<Materials> = (MaterialGroup Material+) *
<Node> = Object3d | Light | Camera | Model
<Mesh> = SkinBones? Geometry Topology PassthroughGP TopologyIP
<Animation> = { All the *Controllers, presumably }
```