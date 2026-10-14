//! Reading a binary FBX file: the model of a weapon that is not the
//! install's (`crate::custom`).
//!
//! The format is Autodesk's and is not published by them; what is here
//! follows the account of it that Blender's developers wrote from the
//! files ("FBX binary file format specification", code.blender.org) and
//! the transform rules of the FBX SDK's own documentation ("Computing
//! transformation matrices"). A file is a tree of records, each with a
//! name, a list of typed values and records of its own; the scene is in
//! the records `Objects` (models, geometries, materials) and
//! `Connections` (which hangs from which).

use std::collections::HashMap;

use anyhow::{anyhow, bail, ensure, Context, Result};

/// One value of a record.
#[derive(Debug, Clone)]
pub enum Prop {
    Int(i64),
    Float(f64),
    Text(String),
    Raw(Vec<u8>),
    Ints(Vec<i64>),
    Floats(Vec<f64>),
}

impl Prop {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Prop::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Prop::Float(v) => Some(*v),
            Prop::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Prop::Text(v) => Some(v),
            _ => None,
        }
    }
}

/// A record of the file.
#[derive(Debug, Clone, Default)]
pub struct Record {
    pub name: String,
    pub props: Vec<Prop>,
    pub children: Vec<Record>,
}

impl Record {
    pub fn child(&self, name: &str) -> Option<&Record> {
        self.children.iter().find(|c| c.name == name)
    }

    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Record> {
        self.children.iter().filter(move |c| c.name == name)
    }

    fn floats(&self, name: &str) -> Option<&[f64]> {
        match self.child(name)?.props.first()? {
            Prop::Floats(v) => Some(v),
            _ => None,
        }
    }

    fn ints(&self, name: &str) -> Option<&[i64]> {
        match self.child(name)?.props.first()? {
            Prop::Ints(v) => Some(v),
            _ => None,
        }
    }

    fn text(&self, name: &str) -> Option<&str> {
        self.child(name)?.props.first()?.as_str()
    }

    /// A property of the record's `Properties70` list, by its name: the
    /// values after the four words that say what kind it is.
    pub fn property(&self, name: &str) -> Option<&[Prop]> {
        self.child("Properties70")?.all("P").find(|p| p.props.first().and_then(Prop::as_str) == Some(name)).map(|p| &p.props[4.min(p.props.len())..])
    }

    fn property_vec3(&self, name: &str) -> Option<[f64; 3]> {
        let values = self.property(name)?;
        Some([values.first()?.as_f64()?, values.get(1)?.as_f64()?, values.get(2)?.as_f64()?])
    }
}

struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
    /// Whether the file's offsets are eight bytes wide (version 7500 on).
    wide: bool,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let bytes = self.data.get(self.at..self.at + count).ok_or_else(|| anyhow!("the file ends inside a record at {}", self.at))?;
        self.at += count;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn offset(&mut self) -> Result<usize> {
        Ok(if self.wide { self.u64()? as usize } else { self.u32()? as usize })
    }

    /// An array's bytes, inflated if the file deflated them.
    fn array(&mut self, width: usize) -> Result<Vec<u8>> {
        let (count, encoding, stored) = (self.u32()? as usize, self.u32()?, self.u32()? as usize);
        let bytes = self.take(stored)?;
        let bytes = match encoding {
            0 => bytes.to_vec(),
            1 => miniz_oxide::inflate::decompress_to_vec_zlib(bytes).map_err(|e| anyhow!("an array could not be inflated: {e}"))?,
            other => bail!("an array is stored in an unknown way ({other})"),
        };
        ensure!(bytes.len() == count * width, "an array of {count} values has {} bytes", bytes.len());
        Ok(bytes)
    }

    fn prop(&mut self) -> Result<Prop> {
        let kind = self.u8()?;
        let chunks = |bytes: Vec<u8>, width: usize| bytes.chunks_exact(width).map(<[u8]>::to_vec).collect::<Vec<_>>();
        Ok(match kind {
            b'C' => Prop::Int(self.u8()? as i64),
            b'Y' => Prop::Int(i16::from_le_bytes(self.take(2)?.try_into().unwrap()) as i64),
            b'I' => Prop::Int(i32::from_le_bytes(self.take(4)?.try_into().unwrap()) as i64),
            b'L' => Prop::Int(i64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            b'F' => Prop::Float(f32::from_le_bytes(self.take(4)?.try_into().unwrap()) as f64),
            b'D' => Prop::Float(f64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            b'S' => {
                let length = self.u32()? as usize;
                Prop::Text(String::from_utf8_lossy(self.take(length)?).into_owned())
            }
            b'R' => {
                let length = self.u32()? as usize;
                Prop::Raw(self.take(length)?.to_vec())
            }
            b'b' => Prop::Ints(self.array(1)?.into_iter().map(i64::from).collect()),
            b'i' => Prop::Ints(chunks(self.array(4)?, 4).iter().map(|b| i32::from_le_bytes(b[..].try_into().unwrap()) as i64).collect()),
            b'l' => Prop::Ints(chunks(self.array(8)?, 8).iter().map(|b| i64::from_le_bytes(b[..].try_into().unwrap())).collect()),
            b'f' => Prop::Floats(chunks(self.array(4)?, 4).iter().map(|b| f32::from_le_bytes(b[..].try_into().unwrap()) as f64).collect()),
            b'd' => Prop::Floats(chunks(self.array(8)?, 8).iter().map(|b| f64::from_le_bytes(b[..].try_into().unwrap())).collect()),
            other => bail!("a value of an unknown kind ({:?}) at {}", other as char, self.at - 1),
        })
    }

    /// The next record, or none at the empty one that ends a list.
    fn record(&mut self) -> Result<Option<Record>> {
        let (end, count, _length) = (self.offset()?, self.offset()?, self.offset()?);
        let name_length = self.u8()? as usize;
        let name = String::from_utf8_lossy(self.take(name_length)?).into_owned();
        if end == 0 {
            return Ok(None);
        }
        let props = (0..count).map(|_| self.prop()).collect::<Result<Vec<_>>>().with_context(|| format!("in the record {name}"))?;
        let mut children = Vec::new();
        while self.at < end {
            match self.record()? {
                Some(child) => children.push(child),
                None => break,
            }
        }
        ensure!(self.at <= end, "the record {name} runs past its end");
        self.at = end;
        Ok(Some(Record { name, props, children }))
    }
}

/// Reads a binary FBX file's records. The root has the file's top records.
pub fn parse(data: &[u8]) -> Result<Record> {
    ensure!(data.starts_with(b"Kaydara FBX Binary  \0"), "not a binary FBX file");
    let version = u32::from_le_bytes(data.get(23..27).ok_or_else(|| anyhow!("the file is too short"))?.try_into().unwrap());
    let mut cursor = Cursor { data, at: 27, wide: version >= 7500 };
    let mut root = Record { name: format!("FBX {version}"), ..Record::default() };
    while let Some(record) = cursor.record()? {
        root.children.push(record);
    }
    Ok(root)
}

type Mat = [[f64; 4]; 4];

const IDENTITY: Mat = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

fn mul(a: Mat, b: Mat) -> Mat {
    std::array::from_fn(|r| std::array::from_fn(|c| (0..4).map(|k| a[r][k] * b[k][c]).sum()))
}

fn translation(t: [f64; 3]) -> Mat {
    let mut m = IDENTITY;
    (m[0][3], m[1][3], m[2][3]) = (t[0], t[1], t[2]);
    m
}

fn scaling(s: [f64; 3]) -> Mat {
    let mut m = IDENTITY;
    (m[0][0], m[1][1], m[2][2]) = (s[0], s[1], s[2]);
    m
}

/// A turn by angles in degrees about x, then y, then z, each about the
/// fixed axes: the format's default order.
fn euler(degrees: [f64; 3]) -> Mat {
    let [(sx, cx), (sy, cy), (sz, cz)] = degrees.map(|d| d.to_radians().sin_cos());
    let x = [[1.0, 0.0, 0.0, 0.0], [0.0, cx, -sx, 0.0], [0.0, sx, cx, 0.0], [0.0, 0.0, 0.0, 1.0]];
    let y = [[cy, 0.0, sy, 0.0], [0.0, 1.0, 0.0, 0.0], [-sy, 0.0, cy, 0.0], [0.0, 0.0, 0.0, 1.0]];
    let z = [[cz, -sz, 0.0, 0.0], [sz, cz, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
    mul(z, mul(y, x))
}

/// The inverse of a matrix that only turns.
fn transposed(m: Mat) -> Mat {
    std::array::from_fn(|r| std::array::from_fn(|c| m[c][r]))
}

fn point(m: Mat, p: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3])
}

fn direction(m: Mat, d: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|r| m[r][0] * d[0] + m[r][1] * d[1] + m[r][2] * d[2])
}

/// A model of the scene: a node of its tree, perhaps with a mesh.
#[derive(Debug, Clone)]
pub struct Model {
    pub name: String,
    pub parent: Option<usize>,
    /// The node's place in the scene, in the file's units and axes.
    pub world: Mat,
    /// Its mesh, in the scene's axes, if it has one.
    pub mesh: Option<Mesh>,
    /// The names of its materials, in the order its mesh counts them.
    pub materials: Vec<String>,
}

/// A mesh as triangles, every corner with its own normal and texture
/// coordinate, in the scene's axes and units.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// The material of each triangle, counted in the model's own list.
    pub materials: Vec<u32>,
    /// Which of the mesh's connected pieces each triangle belongs to
    /// (`pieces` of them): triangles that share a corner's place are one
    /// piece. A model made in one mesh has its moving parts as pieces.
    pub piece: Vec<u32>,
    pub pieces: u32,
}

/// A scene: its models, parents before children, and what its axes are.
#[derive(Debug, Clone)]
pub struct Scene {
    pub models: Vec<Model>,
    /// Centimetres to a unit of the file.
    pub unit: f64,
    /// The axis that is up, the one that is to the front and the third
    /// (0 x, 1 y, 2 z), each with its sign.
    pub up: (usize, f64),
    pub front: (usize, f64),
    pub coord: (usize, f64),
}

/// One layer of a geometry's values (normals or texture coordinates) laid
/// out on the corners of its polygons.
fn layer<const N: usize>(geometry: &Record, element: &str, values: &str, indices: &str, corners: &[i64], polygon_of: &[usize]) -> Option<Vec<[f64; N]>> {
    let element = geometry.child(element)?;
    let data = element.floats(values)?;
    let index = element.ints(indices);
    let direct = element.text("ReferenceInformationType").is_none_or(|kind| kind == "Direct");
    let mapping = element.text("MappingInformationType").unwrap_or("ByPolygonVertex");
    let value = |at: usize| -> Option<[f64; N]> {
        let at = if direct { at } else { usize::try_from(*index?.get(at)?).ok()? };
        let slice = data.get(at * N..at * N + N)?;
        Some(std::array::from_fn(|k| slice[k]))
    };
    (0..corners.len())
        .map(|corner| match mapping {
            "ByPolygonVertex" => value(corner),
            "ByVertice" | "ByVertex" | "ByControlPoint" => value(corners[corner] as usize),
            "ByPolygon" => value(polygon_of[corner]),
            _ => value(0),
        })
        .collect()
}

fn mesh_of(geometry: &Record, place: Mat, turn: Mat) -> Result<Mesh> {
    let points = geometry.floats("Vertices").ok_or_else(|| anyhow!("a geometry has no vertices"))?;
    let raw = geometry.ints("PolygonVertexIndex").ok_or_else(|| anyhow!("a geometry has no polygons"))?;
    // A polygon's last corner is stored as its complement.
    let corners: Vec<i64> = raw.iter().map(|&i| if i < 0 { !i } else { i }).collect();
    let mut polygon_of = Vec::with_capacity(raw.len());
    let mut polygons: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    for (at, &i) in raw.iter().enumerate() {
        polygon_of.push(polygons.len());
        if i < 0 {
            polygons.push((start, at + 1));
            start = at + 1;
        }
    }
    let normals = layer::<3>(geometry, "LayerElementNormal", "Normals", "NormalsIndex", &corners, &polygon_of);
    let uvs = layer::<2>(geometry, "LayerElementUV", "UV", "UVIndex", &corners, &polygon_of);
    let material_of = |polygon: usize| -> u32 {
        let Some(element) = geometry.child("LayerElementMaterial") else { return 0 };
        let Some(list) = element.ints("Materials") else { return 0 };
        let all_same = element.text("MappingInformationType") == Some("AllSame");
        list.get(if all_same { 0 } else { polygon }).copied().unwrap_or(0).max(0) as u32
    };

    // The connected pieces, by the corners' own points.
    let count = points.len() / 3;
    let mut group: Vec<usize> = (0..count).collect();
    fn find(group: &mut [usize], mut at: usize) -> usize {
        while group[at] != at {
            group[at] = group[group[at]];
            at = group[at];
        }
        at
    }
    for &(from, to) in &polygons {
        let first = find(&mut group, corners[from] as usize);
        for corner in from + 1..to {
            let other = find(&mut group, corners[corner] as usize);
            group[other] = first;
        }
    }
    let mut numbered: HashMap<usize, u32> = HashMap::new();

    let mut mesh = Mesh::default();
    for (polygon, &(from, to)) in polygons.iter().enumerate() {
        let root = find(&mut group, corners[from] as usize);
        let next = numbered.len() as u32;
        let piece = *numbered.entry(root).or_insert(next);
        // A fan from the polygon's first corner.
        for second in from + 1..to.saturating_sub(1) {
            for corner in [from, second, second + 1] {
                let at = corners[corner] as usize;
                let p = points.get(at * 3..at * 3 + 3).ok_or_else(|| anyhow!("a polygon names a vertex the geometry has not"))?;
                mesh.positions.push(point(place, [p[0], p[1], p[2]]).map(|v| v as f32));
                let n = normals.as_ref().map_or([0.0, 0.0, 0.0], |n| direction(turn, n[corner]));
                mesh.normals.push(n.map(|v| v as f32));
                mesh.uvs.push(uvs.as_ref().map_or([0.0, 0.0], |uv| uv[corner].map(|v| v as f32)));
            }
            mesh.materials.push(material_of(polygon));
            mesh.piece.push(piece);
        }
    }
    mesh.pieces = numbered.len() as u32;
    Ok(mesh)
}

/// Reads a file's scene: every model with its place and its mesh.
pub fn read(data: &[u8]) -> Result<Scene> {
    let root = parse(data)?;
    let objects = root.child("Objects").ok_or_else(|| anyhow!("the file has no objects"))?;
    let id_of = |record: &Record| record.props.first().and_then(Prop::as_i64).unwrap_or(0);
    // A name is stored with its class after it, a zero and a one between.
    let name_of = |record: &Record| record.props.get(1).and_then(Prop::as_str).map_or(String::new(), |name| name.split('\0').next().unwrap_or("").to_owned());

    let mut parent_of: HashMap<i64, Vec<i64>> = HashMap::new();
    for link in root.child("Connections").into_iter().flat_map(|c| c.all("C")) {
        if link.props.first().and_then(Prop::as_str) == Some("OO") {
            if let (Some(child), Some(parent)) = (link.props.get(1).and_then(Prop::as_i64), link.props.get(2).and_then(Prop::as_i64)) {
                parent_of.entry(child).or_default().push(parent);
            }
        }
    }
    let records: Vec<&Record> = objects.all("Model").collect();
    let index_of: HashMap<i64, usize> = records.iter().enumerate().map(|(index, record)| (id_of(record), index)).collect();
    let parent = |index: usize| parent_of.get(&id_of(records[index])).and_then(|parents| parents.iter().find_map(|p| index_of.get(p).copied()));

    // A node's place in its parent (the SDK's rule, with the default order
    // of turning and no inherited-scale special cases), and the place of
    // its geometry in the node.
    let local = |record: &Record| -> Mat {
        let get = |name: &str, default: f64| record.property_vec3(name).unwrap_or([default; 3]);
        let (pivot, scale_pivot) = (get("RotationPivot", 0.0), get("ScalingPivot", 0.0));
        let negative = |v: [f64; 3]| v.map(|x| -x);
        [
            translation(get("Lcl Translation", 0.0)),
            translation(get("RotationOffset", 0.0)),
            translation(pivot),
            euler(get("PreRotation", 0.0)),
            euler(get("Lcl Rotation", 0.0)),
            transposed(euler(get("PostRotation", 0.0))),
            translation(negative(pivot)),
            translation(get("ScalingOffset", 0.0)),
            translation(scale_pivot),
            scaling(get("Lcl Scaling", 1.0)),
            translation(negative(scale_pivot)),
        ]
        .into_iter()
        .fold(IDENTITY, mul)
    };
    let geometric = |record: &Record| -> (Mat, Mat) {
        let get = |name: &str, default: f64| record.property_vec3(name).unwrap_or([default; 3]);
        let turn = euler(get("GeometricRotation", 0.0));
        (mul(translation(get("GeometricTranslation", 0.0)), mul(turn, scaling(get("GeometricScaling", 1.0)))), turn)
    };

    // Parents before children.
    let mut order: Vec<usize> = Vec::with_capacity(records.len());
    let mut placed = vec![usize::MAX; records.len()];
    while order.len() < records.len() {
        let before = order.len();
        for index in 0..records.len() {
            if placed[index] == usize::MAX && parent(index).is_none_or(|p| placed[p] != usize::MAX) {
                placed[index] = order.len();
                order.push(index);
            }
        }
        ensure!(order.len() > before, "the scene's models hang from one another in a ring");
    }

    let geometries: HashMap<i64, &Record> = objects.all("Geometry").map(|record| (id_of(record), record)).collect();
    let materials: HashMap<i64, String> = objects.all("Material").map(|record| (id_of(record), name_of(record))).collect();
    let mut models: Vec<Model> = Vec::with_capacity(records.len());
    for &index in &order {
        let record = records[index];
        let parent = parent(index).map(|p| placed[p]);
        let world = mul(parent.map_or(IDENTITY, |p| models[p].world), local(record));
        // What hangs from this model: its geometry, and its materials in
        // the order the file connects them.
        let id = id_of(record);
        let hung = |wanted: &dyn Fn(i64) -> bool| -> Vec<i64> {
            root.child("Connections")
                .into_iter()
                .flat_map(|c| c.all("C"))
                .filter(|link| link.props.first().and_then(Prop::as_str) == Some("OO") && link.props.get(2).and_then(Prop::as_i64) == Some(id))
                .filter_map(|link| link.props.get(1).and_then(Prop::as_i64))
                .filter(|child| wanted(*child))
                .collect()
        };
        let mesh = match hung(&|child| geometries.contains_key(&child)).first() {
            Some(geometry) => {
                let (place, turn) = geometric(record);
                // Normals go by the turn alone: the scales met so far are even.
                let whole = mul(world, place);
                let mut turning = mul(world, turn);
                for column in 0..3 {
                    let length = (0..3).map(|r| turning[r][column] * turning[r][column]).sum::<f64>().sqrt().max(1e-12);
                    (0..3).for_each(|r| turning[r][column] /= length);
                }
                Some(mesh_of(geometries[geometry], whole, turning).with_context(|| format!("the mesh of {}", name_of(record)))?)
            }
            None => None,
        };
        let material_names = hung(&|child| materials.contains_key(&child)).iter().map(|m| materials[m].clone()).collect();
        models.push(Model { name: name_of(record), parent, world, mesh, materials: material_names });
    }

    let settings = root.child("GlobalSettings");
    let setting = |name: &str, default: f64| settings.and_then(|s| s.property(name)).and_then(|v| v.first()?.as_f64()).unwrap_or(default);
    let axis = |name: &str, default: usize| (setting(name, default as f64) as usize, setting(&format!("{name}Sign"), 1.0));
    Ok(Scene { models, unit: setting("UnitScaleFactor", 1.0), up: axis("UpAxis", 1), front: axis("FrontAxis", 2), coord: axis("CoordAxis", 0) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_about_each_axis_in_the_formats_order() {
        // A quarter turn about x takes y to z; then one about z takes x to y.
        let m = euler([90.0, 0.0, 90.0]);
        let y = direction(m, [0.0, 1.0, 0.0]);
        assert!((y[2] - 1.0).abs() < 1e-9, "{y:?}");
        let x = direction(m, [1.0, 0.0, 0.0]);
        assert!((x[1] - 1.0).abs() < 1e-9, "{x:?}");
    }

    #[test]
    fn a_file_that_is_not_one_is_refused() {
        assert!(parse(b"not an fbx").is_err());
    }
}
