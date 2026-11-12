//! The shared PhysX engine, the cooked floor and the layers' filter.

use super::*;

/// The engine itself, one for the whole program as PhysX allows (one
/// foundation a process): its foundation, its physics, which makes every
/// world's objects, and its cooking of convex meshes. Made when a world is
/// first wanted and kept to the end.
pub(super) struct Engine {
    pub(super) physics: *mut PxPhysics,
    pub(super) cooking: *mut PxCooking,
}

// Made once and never changed; what it makes is reached through a world.
unsafe impl Send for Engine {}

unsafe impl Sync for Engine {}

pub(super) fn engine() -> &'static Engine {
    static ENGINE: std::sync::OnceLock<Engine> = std::sync::OnceLock::new();
    ENGINE.get_or_init(|| unsafe {
        let foundation = physx_create_foundation();
        let physics = physx_create_physics(foundation);
        phys_PxInitExtensions(physics, null_mut());
        let params = PxCookingParams_new(PxPhysics_getTolerancesScale(physics));
        // PhysX 4.1.1's version, as the cooking asks for it.
        let cooking = phys_PxCreateCooking((4 << 24) + (1 << 16) + (1 << 8), foundation, &params);
        Engine { physics, cooking }
    })
}

/// The floor's size (`World::add_floor`): `FLOOR_HALF` metres each way from
/// its point, in squares of `FLOOR_CELL` (a level's floor is many smaller
/// triangles; that their size changes nothing of a meeting is
/// `PROVISIONAL`).
pub const FLOOR_HALF: f32 = 2048.0;

pub const FLOOR_CELL: f32 = 32.0;

/// The floor's mesh cooked now (some 30 ms, once for the program), not on
/// the first world that wants it.
pub fn cook_floor() {
    floor_mesh();
}

/// The floor's mesh, cooked once for the program and shared by every
/// world's floor.
pub(super) fn floor_mesh() -> *mut PxTriangleMesh {
    static FLOOR: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *FLOOR.get_or_init(|| unsafe {
        let n = (2.0 * FLOOR_HALF / FLOOR_CELL) as u32;
        let mut points: Vec<V3> = Vec::with_capacity(((n + 1) * (n + 1)) as usize);
        for i in 0..=n {
            for j in 0..=n {
                points.push([-FLOOR_HALF + i as f32 * FLOOR_CELL, 0.0, -FLOOR_HALF + j as f32 * FLOOR_CELL]);
            }
        }
        let corner = |i: u32, j: u32| i * (n + 1) + j;
        // Each square's two triangles, facing up (their corners' cross
        // product points up: PhysX's face).
        let mut triangles: Vec<[u32; 3]> = Vec::with_capacity((2 * n * n) as usize);
        for i in 0..n {
            for j in 0..n {
                triangles.push([corner(i, j), corner(i, j + 1), corner(i + 1, j + 1)]);
                triangles.push([corner(i, j), corner(i + 1, j + 1), corner(i + 1, j)]);
            }
        }
        let engine = engine();
        let mut desc = PxTriangleMeshDesc_new();
        desc.points.count = points.len() as u32;
        desc.points.stride = std::mem::size_of::<V3>() as u32;
        desc.points.data = points.as_ptr() as *const std::ffi::c_void;
        desc.triangles.count = triangles.len() as u32;
        desc.triangles.stride = std::mem::size_of::<[u32; 3]>() as u32;
        desc.triangles.data = triangles.as_ptr() as *const std::ffi::c_void;
        let mut result = PxTriangleMeshCookingResult::eSUCCESS;
        let mesh = PxCooking_createTriangleMesh(engine.cooking, &desc, PxPhysics_getPhysicsInsertionCallback_mut(engine.physics), &mut result);
        assert!(!mesh.is_null(), "the floor's mesh could not be cooked ({result:?})");
        mesh as usize
    }) as *mut PxTriangleMesh
}

/// Which pairs of shapes touch (Unity's own filter, as far as it shows):
/// those whose layers meet in the matrix (PhysX's groups, set from it);
/// a trigger is told of what enters it and pushes nothing; every other
/// pair touches, and a body that asks for continuous detection is swept
/// against it.
pub(super) unsafe extern "C" fn layer_filter(info: *mut FilterShaderCallbackInfo) -> u16 {
    let info = &mut *info;
    if !phys_PxGetGroupCollisionFlag(info.filterData0.word0 as u16, info.filterData1.word0 as u16) {
        return PxFilterFlag::eSUPPRESS as u16;
    }
    (*info.pairFlags).mBits = if phys_PxFilterObjectIsTrigger(info.attributes0) || phys_PxFilterObjectIsTrigger(info.attributes1) {
        PxPairFlag::eTRIGGER_DEFAULT as u16
    } else {
        // A pair with a body that reports its touches is told of when it
        // first touches, and where (asked for alone: it changes nothing of
        // the pair's motion).
        let report = (info.filterData0.word1 | info.filterData1.word1) & REPORTS_TOUCHES != 0;
        let told = if report { PxPairFlag::eNOTIFY_TOUCH_FOUND | PxPairFlag::eNOTIFY_TOUCH_CCD | PxPairFlag::eNOTIFY_CONTACT_POINTS } else { 0 };
        (PxPairFlag::eCONTACT_DEFAULT | PxPairFlag::eDETECT_CCD_CONTACT | told) as u16
    };
    PxFilterFlag::eDEFAULT as u16
}
