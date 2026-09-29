//! Procedural 3D models: a model is a list of primitive parts blended together as a
//! signed distance field (smooth union = "morph"), then meshed with surface nets and
//! shaded with smooth normals from the field gradient. No triangle edges visible.

use crate::spec::{ModelPart, ModelSpec, PartShape};
use glam::{Quat, Vec3, Vec3Swizzles};

#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

struct Part {
    shape: PartShape,
    pos: Vec3,
    size: Vec3,
    inv_rot: Quat,
    blend: f32,
    color: Vec3,
    subtract: bool,
}

impl Part {
    fn from_spec(p: &ModelPart) -> Self {
        let rot = Quat::from_euler(
            glam::EulerRot::XYZ,
            p.rotation[0].to_radians(),
            p.rotation[1].to_radians(),
            p.rotation[2].to_radians(),
        );
        Part {
            shape: p.shape,
            pos: Vec3::from(p.position),
            size: Vec3::from(p.size).max(Vec3::splat(0.01)),
            inv_rot: rot.inverse(),
            blend: p.blend.max(0.0),
            color: Vec3::from(p.color),
            subtract: p.subtract,
        }
    }

    fn distance(&self, world: Vec3) -> f32 {
        let q = self.inv_rot * (world - self.pos);
        let h = self.size * 0.5;
        match self.shape {
            PartShape::Sphere => {
                // Ellipsoid approximation: scale space, correct by min radius.
                let k = (q / h).length();
                (k - 1.0) * h.min_element()
            }
            PartShape::Box => {
                let d = q.abs() - h;
                d.max(Vec3::ZERO).length() + d.max_element().min(0.0)
            }
            PartShape::Capsule => {
                // Vertical capsule: radius from x/z, total height = size.y.
                let r = h.x.min(h.z);
                let half = (h.y - r).max(0.0);
                let y = q.y.clamp(-half, half);
                (q - Vec3::new(0.0, y, 0.0)).length() - r
            }
            PartShape::Cylinder => {
                let r = h.x.min(h.z);
                let d = glam::Vec2::new(glam::Vec2::new(q.x, q.z).length() - r, q.y.abs() - h.y);
                d.max(glam::Vec2::ZERO).length() + d.max_element().min(0.0)
            }
            PartShape::Cone => {
                // Base radius at bottom, tip at top.
                let r = h.x.min(h.z);
                let t = ((q.y + h.y) / (2.0 * h.y)).clamp(0.0, 1.0);
                let rr = r * (1.0 - t);
                let radial = glam::Vec2::new(q.x, q.z).length() - rr;
                let vertical = q.y.abs() - h.y;
                radial.max(vertical) * 0.8
            }
        }
    }
}

fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

fn smooth_max(a: f32, b: f32, k: f32) -> f32 {
    -smooth_min(-a, -b, k)
}

struct Field {
    parts: Vec<Part>,
}

impl Field {
    fn eval(&self, p: Vec3) -> f32 {
        let mut d = f32::INFINITY;
        for part in self.parts.iter().filter(|p| !p.subtract) {
            let pd = part.distance(p);
            d = if d.is_infinite() { pd } else { smooth_min(d, pd, part.blend) };
        }
        for part in self.parts.iter().filter(|p| p.subtract) {
            d = smooth_max(d, -part.distance(p), part.blend);
        }
        d
    }

    /// Colour = blend of part colours weighted by proximity to each part surface.
    fn color(&self, p: Vec3) -> Vec3 {
        let mut acc = Vec3::ZERO;
        let mut wsum = 0.0;
        for part in self.parts.iter().filter(|p| !p.subtract) {
            let d = part.distance(p).max(0.0);
            let w = 1.0 / (d * d * 40.0 + 0.02);
            acc += part.color * w;
            wsum += w;
        }
        if wsum > 0.0 { acc / wsum } else { Vec3::ONE }
    }

    fn normal(&self, p: Vec3, eps: f32) -> Vec3 {
        let e = Vec3::new(eps, 0.0, 0.0);
        let n = Vec3::new(
            self.eval(p + e) - self.eval(p - e),
            self.eval(p + e.yxy()) - self.eval(p - e.yxy()),
            self.eval(p + e.yyx()) - self.eval(p - e.yyx()),
        );
        n.normalize_or(Vec3::Y)
    }
}

#[derive(Clone, Debug)]
pub struct GroupMesh {
    pub name: String,
    pub pivot: Vec3,
    pub mesh: MeshData,
}

/// Vertical bounds (min_y, max_y) of all additive parts, in model units.
pub fn bounds_y(spec: &ModelSpec) -> (f32, f32) {
    if let Some(m) = &spec.imported {
        return (0.0, m.size.y.max(1e-4));
    }
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for p in spec.parts.iter().filter(|p| !p.subtract) {
        let h = p.size[1] * 0.5;
        lo = lo.min(p.position[1] - h);
        hi = hi.max(p.position[1] + h);
    }
    if lo.is_finite() && hi > lo { (lo, hi) } else { (0.0, 1.0) }
}

/// Builds one mesh per animation group. Parts without a group go into "body".
/// Subtractive parts apply to the group they are declared in.
pub fn build_groups(spec: &ModelSpec) -> Vec<GroupMesh> {
    if let Some(m) = &spec.imported {
        return m.groups.clone();
    }
    let mut order: Vec<String> = Vec::new();
    for p in &spec.parts {
        let g = p.group.clone().unwrap_or_else(|| "body".into());
        if !order.contains(&g) {
            order.push(g);
        }
    }
    order
        .into_iter()
        .map(|g| {
            let parts: Vec<ModelPart> = spec.parts.iter().filter(|p| p.group.clone().unwrap_or_else(|| "body".into()) == g).cloned().collect();
            let sub = ModelSpec { parts: parts.clone(), resolution: spec.resolution, ..Default::default() };
            let mesh = build(&sub);
            // Pivot: explicit, else by convention from the group's bounds.
            let pivot = parts.iter().find_map(|p| p.pivot).map(Vec3::from).unwrap_or_else(|| {
                let mut lo = Vec3::splat(f32::INFINITY);
                let mut hi = Vec3::splat(f32::NEG_INFINITY);
                for p in parts.iter().filter(|p| !p.subtract) {
                    let h = Vec3::from(p.size) * 0.5;
                    lo = lo.min(Vec3::from(p.position) - h);
                    hi = hi.max(Vec3::from(p.position) + h);
                }
                let c = (lo + hi) * 0.5;
                let gl = g.to_ascii_lowercase();
                if gl.starts_with("leg") || gl.starts_with("arm") || gl.starts_with("tail") || gl.starts_with("ear") {
                    Vec3::new(c.x, hi.y, c.z)
                } else if gl.starts_with("head") || gl.starts_with("hat") {
                    Vec3::new(c.x, lo.y, c.z)
                } else if gl.starts_with("body") {
                    Vec3::new(c.x, lo.y, c.z)
                } else {
                    c
                }
            });
            GroupMesh { name: g, pivot, mesh }
        })
        .collect()
}

/// Builds a mesh for a model spec. Output is centred as authored (parts are in model units;
/// an entity's `scale` multiplies them).
pub fn build(spec: &ModelSpec) -> MeshData {
    let parts: Vec<Part> = spec.parts.iter().map(Part::from_spec).collect();
    let field = Field { parts };
    if field.parts.is_empty() {
        return MeshData { positions: vec![], normals: vec![], colors: vec![], indices: vec![] };
    }

    // Bounds from parts (+ blend margin).
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for p in field.parts.iter().filter(|p| !p.subtract) {
        let r = p.size.length() * 0.5 + p.blend;
        lo = lo.min(p.pos - Vec3::splat(r));
        hi = hi.max(p.pos + Vec3::splat(r));
    }
    let margin = 0.1;
    lo -= Vec3::splat(margin);
    hi += Vec3::splat(margin);
    let res = spec.resolution.clamp(8, 96) as usize;
    let extent = hi - lo;
    let cell = extent.max_element() / res as f32;
    let nx = ((extent.x / cell).ceil() as usize + 1).max(2);
    let ny = ((extent.y / cell).ceil() as usize + 1).max(2);
    let nz = ((extent.z / cell).ceil() as usize + 1).max(2);

    // Sample the field on the grid corners.
    let idx = |x: usize, y: usize, z: usize| (x * ny + y) * nz + z;
    let mut samples = vec![0.0f32; nx * ny * nz];
    for x in 0..nx {
        for y in 0..ny {
            for z in 0..nz {
                let p = lo + Vec3::new(x as f32, y as f32, z as f32) * cell;
                samples[idx(x, y, z)] = field.eval(p);
            }
        }
    }

    // Surface nets: one vertex per cell that has a sign change.
    let cells = (nx - 1) * (ny - 1) * (nz - 1);
    let mut cell_vertex = vec![u32::MAX; cells];
    let cidx = |x: usize, y: usize, z: usize| (x * (ny - 1) + y) * (nz - 1) + z;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    const EDGES: [(usize, usize); 12] = [
        (0, 1), (2, 3), (4, 5), (6, 7), // x edges
        (0, 2), (1, 3), (4, 6), (5, 7), // y edges
        (0, 4), (1, 5), (2, 6), (3, 7), // z edges
    ];
    let corner = |c: usize| (c & 1, (c >> 1) & 1, (c >> 2) & 1);
    for x in 0..nx - 1 {
        for y in 0..ny - 1 {
            for z in 0..nz - 1 {
                let mut vals = [0.0f32; 8];
                for c in 0..8 {
                    let (dx, dy, dz) = corner(c);
                    vals[c] = samples[idx(x + dx, y + dy, z + dz)];
                }
                let mut sum = Vec3::ZERO;
                let mut count = 0;
                for (a, b) in EDGES {
                    if (vals[a] < 0.0) != (vals[b] < 0.0) {
                        let t = vals[a] / (vals[a] - vals[b]);
                        let (ax, ay, az) = corner(a);
                        let (bx, by, bz) = corner(b);
                        let pa = Vec3::new(ax as f32, ay as f32, az as f32);
                        let pb = Vec3::new(bx as f32, by as f32, bz as f32);
                        sum += pa.lerp(pb, t);
                        count += 1;
                    }
                }
                if count == 0 {
                    continue;
                }
                let local = sum / count as f32;
                let p = lo + (Vec3::new(x as f32, y as f32, z as f32) + local) * cell;
                cell_vertex[cidx(x, y, z)] = positions.len() as u32;
                positions.push(p.to_array());
                normals.push(field.normal(p, cell * 0.5).to_array());
                colors.push(field.color(p).to_array());
            }
        }
    }

    // Quads across each sign-changing edge, between the 4 cells sharing that edge.
    let mut indices = Vec::new();
    for x in 0..nx - 1 {
        for y in 0..ny - 1 {
            for z in 0..nz - 1 {
                let v0 = samples[idx(x, y, z)];
                // Edge along +x from (x,y,z): shared by cells (x, y-1..y, z-1..z)
                if x < nx - 1 && y > 0 && z > 0 {
                    let v1 = samples[idx(x + 1, y, z)];
                    if (v0 < 0.0) != (v1 < 0.0) {
                        emit_quad(&mut indices, &cell_vertex, [
                            cidx(x, y, z), cidx(x, y - 1, z), cidx(x, y - 1, z - 1), cidx(x, y, z - 1),
                        ], v0 < 0.0);
                    }
                }
                if y < ny - 1 && x > 0 && z > 0 {
                    let v1 = samples[idx(x, y + 1, z)];
                    if (v0 < 0.0) != (v1 < 0.0) {
                        emit_quad(&mut indices, &cell_vertex, [
                            cidx(x, y, z), cidx(x, y, z - 1), cidx(x - 1, y, z - 1), cidx(x - 1, y, z),
                        ], v0 < 0.0);
                    }
                }
                if z < nz - 1 && x > 0 && y > 0 {
                    let v1 = samples[idx(x, y, z + 1)];
                    if (v0 < 0.0) != (v1 < 0.0) {
                        emit_quad(&mut indices, &cell_vertex, [
                            cidx(x, y, z), cidx(x - 1, y, z), cidx(x - 1, y - 1, z), cidx(x, y - 1, z),
                        ], v0 < 0.0);
                    }
                }
            }
        }
    }

    MeshData { positions, normals, colors, indices }
}

fn emit_quad(indices: &mut Vec<u32>, cell_vertex: &[u32], cells: [usize; 4], flip: bool) {
    let v: Vec<u32> = cells.iter().map(|&c| cell_vertex[c]).collect();
    if v.iter().any(|&i| i == u32::MAX) {
        return;
    }
    let (a, b, c, d) = if flip { (v[0], v[3], v[2], v[1]) } else { (v[0], v[1], v[2], v[3]) };
    indices.extend_from_slice(&[a, b, c, a, c, d]);
}

// ---------------------------------------------------------------- imported meshes (glTF / GLB / OBJ)

/// A mesh file turned into the engine's model form: one flat-shaded mesh per animation group,
/// colours baked into the vertices, feet at y = 0 and centred on x/z.
#[derive(Clone)]
pub struct Imported {
    pub groups: Vec<GroupMesh>,
    /// Extents after normalising (x, y = height, z).
    pub size: Vec3,
    pub triangles: usize,
}

impl std::fmt::Debug for Imported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.groups.iter().map(|g| g.name.as_str()).collect();
        write!(f, "Imported {{ groups: {names:?}, size: {:?}, triangles: {} }}", self.size, self.triangles)
    }
}

/// Animation group for a node name, guessed from common rig names ("arm-left", "UpperArm.L",
/// "legRight", "leg-front-left", "Head"). Quadruped and spider legs alternate between
/// `leg_l` and `leg_r` so the procedural walk moves diagonal pairs together.
pub fn guess_group(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    let has = |w: &str| n.contains(w);
    let tag = |s: &str| n.ends_with(s) || n.contains(&format!("{s}_")) || n.contains(&format!("{s}."));
    let left = has("left") || tag(".l") || tag("_l") || tag("-l") || n.starts_with("l_");
    let right = has("right") || tag(".r") || tag("_r") || tag("-r") || n.starts_with("r_");
    let side = if left && !right {
        Some(true)
    } else if right && !left {
        Some(false)
    } else {
        None
    };
    if has("head") && !has("forehead") {
        return Some("head");
    }
    if has("tail") {
        return Some("tail");
    }
    if ["leg", "foot", "thigh", "shin", "knee", "calf", "toe"].iter().any(|w| has(w)) {
        let swap = has("mid") ^ (has("back") || has("hind") || has("rear"));
        return side.map(|l| if l ^ swap { "leg_l" } else { "leg_r" });
    }
    if ["arm", "shoulder", "hand", "wing", "elbow", "finger", "thumb", "pinky", "index", "middle", "ring"].iter().any(|w| has(w)) {
        return side.map(|l| if l { "arm_l" } else { "arm_r" });
    }
    None
}

struct Tri {
    p: [Vec3; 3],
    color: Vec3,
    group: usize,
}

/// Collects triangles per group, then normalises and builds the meshes.
struct Builder {
    /// (group name, pivot in model space before normalising)
    groups: Vec<(String, Option<Vec3>)>,
    tris: Vec<Tri>,
}

impl Builder {
    fn group(&mut self, name: &str, pivot: Option<Vec3>) -> usize {
        let key = pivot.map(|v| v.to_array());
        if let Some(i) = self.groups.iter().position(|(n, p)| n == name && p.map(|v| v.to_array()) == key) {
            return i;
        }
        self.groups.push((name.to_string(), pivot));
        self.groups.len() - 1
    }

    fn finish(self) -> anyhow::Result<Imported> {
        if self.tris.is_empty() {
            anyhow::bail!("the file has no triangles");
        }
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for t in &self.tris {
            for p in t.p {
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        let shift = Vec3::new(-(lo.x + hi.x) * 0.5, -lo.y, -(lo.z + hi.z) * 0.5);
        let size = hi - lo;
        let mut out = Vec::new();
        for (gi, (name, pivot)) in self.groups.iter().enumerate() {
            let mut mesh = MeshData::default();
            let (mut glo, mut ghi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
            for t in self.tris.iter().filter(|t| t.group == gi) {
                let p = [t.p[0] + shift, t.p[1] + shift, t.p[2] + shift];
                let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or(Vec3::Y);
                for v in &p {
                    glo = glo.min(*v);
                    ghi = ghi.max(*v);
                    mesh.indices.push(mesh.positions.len() as u32);
                    mesh.positions.push(v.to_array());
                    mesh.normals.push(n.to_array());
                    mesh.colors.push(t.color.to_array());
                }
            }
            if mesh.positions.is_empty() {
                continue;
            }
            let c = (glo + ghi) * 0.5;
            let pivot = pivot.map(|p| p + shift).unwrap_or(Vec3::new(c.x, glo.y, c.z));
            out.push(GroupMesh { name: name.clone(), pivot, mesh });
        }
        Ok(Imported { groups: out, size, triangles: self.tris.len() })
    }
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Colour of a texture at `uv` (wrapping), converted to linear.
fn sample(tex: &crate::spec::Rgba, uv: glam::Vec2) -> Vec3 {
    if tex.width == 0 || tex.height == 0 {
        return Vec3::ONE;
    }
    let u = uv.x - uv.x.floor();
    let v = uv.y - uv.y.floor();
    let x = ((u * tex.width as f32) as u32).min(tex.width - 1);
    let y = ((v * tex.height as f32) as u32).min(tex.height - 1);
    let i = ((y * tex.width + x) * 4) as usize;
    let c = |k: usize| srgb_to_linear(tex.pixels[i + k] as f32 / 255.0);
    Vec3::new(c(0), c(1), c(2))
}

/// Decode a texture without the sprite alpha cut-out.
fn decode_texture(bytes: &[u8]) -> anyhow::Result<crate::spec::Rgba> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)?.to_rgba8();
    let (width, height) = img.dimensions();
    Ok(crate::spec::Rgba { width, height, pixels: img.into_raw() })
}

/// Import a mesh file. `groups` maps node names to animation groups (empty = guess);
/// `texture` overrides the colour texture the file refers to.
pub fn import(path: &std::path::Path, groups: &std::collections::BTreeMap<String, String>, texture: Option<&std::path::Path>) -> anyhow::Result<Imported> {
    use anyhow::Context;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let tex = match texture {
        Some(t) => {
            let bytes = std::fs::read(t).with_context(|| format!("cannot read texture {}", t.display()))?;
            Some(decode_texture(&bytes).with_context(|| format!("texture {}", t.display()))?)
        }
        None => None,
    };
    let r = match ext.as_str() {
        "glb" | "gltf" => import_gltf(path, groups, tex),
        "obj" => import_obj(path, tex),
        _ => Err(anyhow::anyhow!("unsupported model format {ext:?} (use .glb, .gltf or .obj)")),
    };
    r.with_context(|| format!("model {}", path.display()))
}

type GroupKey = (String, Option<usize>);

fn import_gltf(path: &std::path::Path, explicit: &std::collections::BTreeMap<String, String>, tex_override: Option<crate::spec::Rgba>) -> anyhow::Result<Imported> {
    use anyhow::{anyhow, Context};
    use glam::{Mat4, Vec2, Vec4};
    let bytes = std::fs::read(path)?;
    let gltf = gltf::Gltf::from_slice(&bytes)?;
    let base = path.parent().unwrap_or(std::path::Path::new("."));
    let buffers = gltf::import_buffers(&gltf.document, Some(base), gltf.blob.clone())?;
    let doc = &gltf.document;

    // Images, decoded when a material first uses them.
    let mut images: Vec<Option<crate::spec::Rgba>> = vec![None; doc.images().count()];
    let mut load_image = |idx: usize| -> anyhow::Result<crate::spec::Rgba> {
        if let Some(img) = &images[idx] {
            return Ok(img.clone());
        }
        let image = doc.images().nth(idx).ok_or_else(|| anyhow!("image {idx} missing"))?;
        let data = match image.source() {
            gltf::image::Source::View { view, .. } => {
                let buf = &buffers[view.buffer().index()];
                buf[view.offset()..view.offset() + view.length()].to_vec()
            }
            gltf::image::Source::Uri { uri, .. } => {
                let file = base.join(percent_decode(uri));
                std::fs::read(&file).with_context(|| format!("cannot read texture {}", file.display()))?
            }
        };
        let img = decode_texture(&data)?;
        images[idx] = Some(img.clone());
        Ok(img)
    };

    // Node hierarchy: parents, rest-pose world matrices and animation groups.
    let n = doc.nodes().count();
    let mut parent = vec![None; n];
    for node in doc.nodes() {
        for c in node.children() {
            parent[c.index()] = Some(node.index());
        }
    }
    let mut world = vec![Mat4::IDENTITY; n];
    let mut group_of: Vec<GroupKey> = vec![("body".into(), None); n];
    let scene = doc.default_scene().or_else(|| doc.scenes().next()).ok_or_else(|| anyhow!("no scene"))?;
    let role = |name: &str| -> Option<String> {
        if explicit.is_empty() {
            guess_group(name).map(|s| s.to_string())
        } else {
            explicit.get(name).cloned()
        }
    };
    let mut order = Vec::new();
    let mut stack: Vec<(gltf::Node, Mat4)> = scene.nodes().map(|nd| (nd, Mat4::IDENTITY)).collect();
    stack.reverse();
    while let Some((node, pm)) = stack.pop() {
        let i = node.index();
        let m = pm * Mat4::from_cols_array_2d(&node.transform().matrix());
        world[i] = m;
        let inherited = parent[i].map(|p| group_of[p].clone()).unwrap_or(("body".into(), None));
        group_of[i] = match role(node.name().unwrap_or("")) {
            Some(r) if r == inherited.0 && inherited.1.is_some() => inherited,
            Some(r) if r == "body" => (r, None),
            Some(r) => (r, Some(i)),
            None => inherited,
        };
        order.push(i);
        let mut kids: Vec<(gltf::Node, Mat4)> = node.children().map(|c| (c, m)).collect();
        kids.reverse();
        stack.extend(kids);
    }

    let mut b = Builder { groups: Vec::new(), tris: Vec::new() };
    let nodes: Vec<gltf::Node> = doc.nodes().collect();
    for &ni in &order {
        let node = &nodes[ni];
        let Some(mesh) = node.mesh() else { continue };
        let skin = node.skin();
        let joint_mats: Vec<Mat4> = match &skin {
            Some(s) => {
                let reader = s.reader(|bf| Some(&buffers[bf.index()]));
                let ibms: Vec<Mat4> = reader
                    .read_inverse_bind_matrices()
                    .map(|it| it.map(|m| Mat4::from_cols_array_2d(&m)).collect())
                    .unwrap_or_else(|| vec![Mat4::IDENTITY; s.joints().count()]);
                s.joints().zip(ibms).map(|(j, ibm)| world[j.index()] * ibm).collect()
            }
            None => Vec::new(),
        };
        let joint_nodes: Vec<usize> = skin.as_ref().map(|s| s.joints().map(|j| j.index()).collect()).unwrap_or_default();
        for prim in mesh.primitives() {
            if prim.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = prim.reader(|bf| Some(&buffers[bf.index()]));
            let Some(pos) = reader.read_positions() else { continue };
            let pos: Vec<Vec3> = pos.map(Vec3::from).collect();
            let vcols: Vec<Vec3> = reader.read_colors(0).map(|c| c.into_rgba_f32().map(|c| Vec3::new(c[0], c[1], c[2])).collect()).unwrap_or_default();
            let joints: Vec<[u16; 4]> = reader.read_joints(0).map(|j| j.into_u16().collect()).unwrap_or_default();
            let weights: Vec<[f32; 4]> = reader.read_weights(0).map(|w| w.into_f32().collect()).unwrap_or_default();
            let idx: Vec<u32> = reader.read_indices().map(|i| i.into_u32().collect()).unwrap_or_else(|| (0..pos.len() as u32).collect());

            let pbr = prim.material().pbr_metallic_roughness();
            let factor = Vec4::from(pbr.base_color_factor()).truncate();
            let (tex, tex_set) = match (&tex_override, pbr.base_color_texture()) {
                (Some(t), info) => (Some(t.clone()), info.map(|i| i.tex_coord()).unwrap_or(0)),
                (None, Some(info)) => (Some(load_image(info.texture().source().index())?), info.tex_coord()),
                (None, None) => (None, 0),
            };
            let uvs: Vec<Vec2> = reader.read_tex_coords(tex_set).map(|t| t.into_f32().map(Vec2::from).collect()).unwrap_or_default();

            // Rest-pose positions.
            let skinned = !joint_mats.is_empty() && joints.len() == pos.len() && weights.len() == pos.len();
            let placed: Vec<Vec3> = if skinned {
                pos.iter()
                    .enumerate()
                    .map(|(v, p)| {
                        let mut acc = Vec3::ZERO;
                        let mut wsum = 0.0;
                        for k in 0..4 {
                            let w = weights[v][k];
                            if w > 0.0 {
                                if let Some(m) = joint_mats.get(joints[v][k] as usize) {
                                    acc += m.transform_point3(*p) * w;
                                    wsum += w;
                                }
                            }
                        }
                        if wsum > 0.0 {
                            acc / wsum
                        } else {
                            world[ni].transform_point3(*p)
                        }
                    })
                    .collect()
            } else {
                pos.iter().map(|p| world[ni].transform_point3(*p)).collect()
            };
            let flip = !skinned && world[ni].determinant() < 0.0;
            // Group of a vertex: its dominant joint's group when skinned, else the node's.
            let vgroup = |v: usize| -> GroupKey {
                if skinned {
                    let k = (0..4).max_by(|a, b| weights[v][*a].total_cmp(&weights[v][*b])).unwrap_or(0);
                    if let Some(&jn) = joint_nodes.get(joints[v][k] as usize) {
                        return group_of[jn].clone();
                    }
                }
                group_of[ni].clone()
            };
            for t in idx.chunks_exact(3) {
                let (a, bb, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
                if a >= placed.len() || bb >= placed.len() || c >= placed.len() {
                    anyhow::bail!("index out of range in mesh {:?}", mesh.name());
                }
                let mut color = factor;
                if let Some(tex) = &tex {
                    if !uvs.is_empty() {
                        color *= sample(tex, (uvs[a] + uvs[bb] + uvs[c]) / 3.0);
                    }
                }
                if !vcols.is_empty() {
                    color *= (vcols[a] + vcols[bb] + vcols[c]) / 3.0;
                }
                let gs = [vgroup(a), vgroup(bb), vgroup(c)];
                let g = if gs[1] == gs[2] && gs[0] != gs[1] { gs[1].clone() } else { gs[0].clone() };
                let pivot = g.1.map(|anchor| world[anchor].transform_point3(Vec3::ZERO));
                let gi = b.group(&g.0, pivot);
                let p = if flip { [placed[a], placed[c], placed[bb]] } else { [placed[a], placed[bb], placed[c]] };
                b.tris.push(Tri { p, color, group: gi });
            }
        }
    }
    b.finish()
}

/// Percent-decoding for texture URIs ("Textures/color%20map.png").
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

type ObjMaterial = (Vec3, Option<crate::spec::Rgba>);

/// Wavefront OBJ with an optional MTL (`Kd` colours, `map_Kd` texture). Objects/groups
/// (`o` / `g`) with limb names become animation groups; the rest is `body`.
fn import_obj(path: &std::path::Path, tex_override: Option<crate::spec::Rgba>) -> anyhow::Result<Imported> {
    use anyhow::Context;
    use glam::Vec2;
    let text = std::fs::read_to_string(path)?;
    let base = path.parent().unwrap_or(std::path::Path::new("."));
    let mut mats: std::collections::HashMap<String, ObjMaterial> = Default::default();
    for line in text.lines() {
        if let Some(lib) = line.trim().strip_prefix("mtllib ") {
            let mtl = std::fs::read_to_string(base.join(lib.trim())).with_context(|| format!("cannot read {}", lib.trim()))?;
            let mut cur = String::new();
            for l in mtl.lines() {
                let l = l.trim();
                if let Some(n) = l.strip_prefix("newmtl ") {
                    cur = n.trim().to_string();
                    mats.insert(cur.clone(), (Vec3::ONE, None));
                } else if let Some(kd) = l.strip_prefix("Kd ") {
                    let v: Vec<f32> = kd.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                    if let (Some(m), true) = (mats.get_mut(&cur), v.len() >= 3) {
                        m.0 = Vec3::new(v[0], v[1], v[2]);
                    }
                } else if let Some(t) = l.strip_prefix("map_Kd ") {
                    let file = base.join(t.trim());
                    let img = decode_texture(&std::fs::read(&file).with_context(|| format!("cannot read texture {}", file.display()))?)?;
                    if let Some(m) = mats.get_mut(&cur) {
                        m.1 = Some(img);
                    }
                }
            }
        }
    }
    let mut v: Vec<Vec3> = Vec::new();
    let mut vt: Vec<Vec2> = Vec::new();
    let mut b = Builder { groups: Vec::new(), tris: Vec::new() };
    let mut group = b.group("body", None);
    let mut mat: ObjMaterial = (Vec3::ONE, None);
    for (ln, line) in text.lines().enumerate() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let c: Vec<f32> = it.filter_map(|x| x.parse().ok()).collect();
                if c.len() < 3 {
                    anyhow::bail!("line {}: bad vertex", ln + 1);
                }
                v.push(Vec3::new(c[0], c[1], c[2]));
            }
            Some("vt") => {
                let c: Vec<f32> = it.filter_map(|x| x.parse().ok()).collect();
                // OBJ v grows upward; images are stored top row first.
                vt.push(Vec2::new(c.first().copied().unwrap_or(0.0), 1.0 - c.get(1).copied().unwrap_or(0.0)));
            }
            Some("o") | Some("g") => {
                let name: String = it.collect::<Vec<_>>().join(" ");
                group = b.group(guess_group(&name).unwrap_or("body"), None);
            }
            Some("usemtl") => {
                let name = it.next().unwrap_or("");
                mat = mats.get(name).cloned().unwrap_or((Vec3::ONE, None));
            }
            Some("f") => {
                let resolve = |s: Option<&str>, len: usize| -> Option<usize> {
                    let i: i64 = s.filter(|s| !s.is_empty())?.parse().ok()?;
                    let r = if i < 0 { len as i64 + i } else { i - 1 };
                    (r >= 0 && (r as usize) < len).then_some(r as usize)
                };
                let mut corners: Vec<(usize, Option<usize>)> = Vec::new();
                for c in it {
                    let mut parts = c.split('/');
                    let vi = resolve(parts.next(), v.len()).ok_or_else(|| anyhow::anyhow!("line {}: bad face", ln + 1))?;
                    corners.push((vi, resolve(parts.next(), vt.len())));
                }
                if corners.len() < 3 {
                    anyhow::bail!("line {}: a face needs 3 corners", ln + 1);
                }
                let tex = tex_override.as_ref().or(mat.1.as_ref());
                for k in 1..corners.len() - 1 {
                    let tri = [corners[0], corners[k], corners[k + 1]];
                    let mut color = mat.0;
                    if let Some(tex) = tex {
                        let uvs: Vec<Vec2> = tri.iter().filter_map(|(_, t)| t.map(|t| vt[t])).collect();
                        if uvs.len() == 3 {
                            color *= sample(tex, (uvs[0] + uvs[1] + uvs[2]) / 3.0);
                        }
                    }
                    b.tris.push(Tri { p: [v[tri[0].0], v[tri[1].0], v[tri[2].0]], color, group });
                }
            }
            _ => {}
        }
    }
    b.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_names_are_guessed() {
        assert_eq!(guess_group("arm-left"), Some("arm_l"));
        assert_eq!(guess_group("armRight"), Some("arm_r"));
        assert_eq!(guess_group("UpperArm.L"), Some("arm_l"));
        assert_eq!(guess_group("leg-right"), Some("leg_r"));
        assert_eq!(guess_group("Head"), Some("head"));
        assert_eq!(guess_group("torso"), None);
        // Diagonal pairs of a quadruped walk together.
        assert_eq!(guess_group("leg-front-left"), Some("leg_l"));
        assert_eq!(guess_group("leg-back-right"), Some("leg_l"));
        assert_eq!(guess_group("leg-back-left"), Some("leg_r"));
    }

    #[test]
    fn obj_is_normalised_feet_at_zero() {
        let dir = std::env::temp_dir().join(format!("droid_obj_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let obj = dir.join("quad.obj");
        std::fs::write(dir.join("quad.mtl"), "newmtl red\nKd 1 0 0\n").unwrap();
        std::fs::write(&obj, "mtllib quad.mtl\nv 1 2 1\nv 3 2 1\nv 3 5 1\nv 1 5 1\nusemtl red\nf 1 2 3 4\n").unwrap();
        let m = import(&obj, &Default::default(), None).unwrap();
        assert_eq!(m.triangles, 2);
        assert_eq!(m.size, Vec3::new(2.0, 3.0, 0.0));
        let g = &m.groups[0];
        assert_eq!(g.name, "body");
        let min_y = g.mesh.positions.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        assert_eq!(min_y, 0.0);
        assert_eq!(g.mesh.colors[0], [1.0, 0.0, 0.0]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
