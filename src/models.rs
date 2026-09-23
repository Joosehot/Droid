//! Procedural 3D models: a model is a list of primitive parts blended together as a
//! signed distance field (smooth union = "morph"), then meshed with surface nets and
//! shaded with smooth normals from the field gradient. No triangle edges visible.

use crate::spec::{ModelPart, ModelSpec, PartShape};
use glam::{Quat, Vec3, Vec3Swizzles};

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

pub struct GroupMesh {
    pub name: String,
    pub pivot: Vec3,
    pub mesh: MeshData,
}

/// Vertical bounds (min_y, max_y) of all additive parts, in model units.
pub fn bounds_y(spec: &ModelSpec) -> (f32, f32) {
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
            let sub = ModelSpec { parts: parts.clone(), resolution: spec.resolution };
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
