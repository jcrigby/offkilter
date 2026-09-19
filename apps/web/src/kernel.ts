// Typed wrapper around the WebAssembly kernel. All document edits are `Op`
// values (see crates/ok-model/src/ops.rs) sent as JSON.

import init, { Studio, version as kernelVersion } from "./wasm/ok_wasm.js";

export type Vec2 = { x: number; y: number };
export type Vec3 = { x: number; y: number; z: number };
export type StandardPlane = "top" | "front" | "right";
/** A face of a body, by the feature that made it and its local face index. */
export type FaceRef = { feature: number; local: number };
export type PlaneRef = { type: "standard"; base: StandardPlane; offset: number } | { type: "face"; face: FaceRef; offset: number };
export type ExtrudeEnd = { type: "blind" } | { type: "through_all" } | { type: "up_to_face"; face: FaceRef };
export type ExtrudeDirection = "normal" | "reverse" | "symmetric";
export type BodyOp = "new" | "add" | "remove" | "intersect";
export type ProfileSelection = { type: "all" } | { type: "largest" } | { type: "indices"; indices: number[] };

export type Constraint =
  | { type: "coincident"; a: number; b: number }
  | { type: "fixed"; point: number }
  | { type: "horizontal"; line: number }
  | { type: "vertical"; line: number }
  | { type: "distance"; a: number; b: number; value: number }
  | { type: "horizontal_distance"; a: number; b: number; value: number }
  | { type: "vertical_distance"; a: number; b: number; value: number }
  | { type: "length"; line: number; value: number }
  | { type: "radius"; entity: number; value: number }
  | { type: "diameter"; entity: number; value: number }
  | { type: "equal"; a: number; b: number }
  | { type: "parallel"; a: number; b: number }
  | { type: "perpendicular"; a: number; b: number }
  | { type: "angle"; a: number; b: number; value: number }
  | { type: "point_on_line"; point: number; line: number }
  | { type: "point_on_circle"; point: number; entity: number }
  | { type: "midpoint"; point: number; line: number }
  | { type: "tangent"; line: number; entity: number };

export type Entity =
  | { type: "point"; pos: Vec2 }
  | { type: "line"; start: number; end: number }
  | { type: "circle"; center: number; radius: number }
  | { type: "arc"; center: number; start: number; end: number };

export type SketchData = {
  entities: ({ id: number } & Entity)[];
  constraints: ({ id: number } & Constraint)[];
  /** Ids of construction entities (absent when none). */
  construction?: number[];
};

export type RevolveAxis = { type: "x_axis" } | { type: "y_axis" } | { type: "line"; line: number };
/** An edge of a body, named by the two faces that meet there. */
export type EdgeRef = { a: FaceRef; b: FaceRef };
export type BlendKind = "fillet" | "chamfer";
export type CopyOp = "add" | "new";
export type Axis = "x" | "y" | "z";
export type PatternKind = { type: "linear"; axis: Axis; spacing: number } | { type: "circular"; axis: Axis; angle: number };
export type FeatureKind =
  | { type: "sketch"; plane: PlaneRef; sketch: SketchData }
  | { type: "extrude"; sketch: number; profiles: ProfileSelection; depth: number; direction: ExtrudeDirection; end: ExtrudeEnd; op: BodyOp }
  | { type: "revolve"; sketch: number; profiles: ProfileSelection; axis: RevolveAxis; angle: number; op: BodyOp }
  | { type: "blend"; kind: BlendKind; edges: EdgeRef[]; size: number }
  | { type: "mirror"; plane: PlaneRef; op: CopyOp }
  | { type: "pattern"; kind: PatternKind; count: number; op: CopyOp }
  | { type: "variable"; name: string; expression: string };

export type FeatureSummary = {
  id: number;
  name: string;
  suppressed: boolean;
  kind: FeatureKind;
  error: string | null;
  /** Expressions bound to numeric fields, by field name. */
  bindings: Record<string, string>;
  /** Evaluated value of a variable feature. */
  value: number | null;
};
export type FaceInfo = { origin: FaceRef; surface: "plane" | "cylinder"; normal: Vec3 };
export type BodySummary = {
  name: string;
  source: number;
  vertices: number;
  triangles: number;
  face_count: number;
  faces: FaceInfo[];
  bounds: [Vec3, Vec3] | null;
  volume: number;
};
export type SolveResult = {
  status: "fully_constrained" | "under_constrained" | "inconsistent";
  iterations: number;
  max_residual: number;
  dof: number;
  equations: number;
  parameters: number;
};
export type SketchCurve = { entity: number; kind: string; construction: boolean; points: Vec3[] };
export type PlaneFrame = { origin: Vec3; x_axis: Vec3; y_axis: Vec3; normal: Vec3 };
export type Loop = { points: Vec2[] };
export type SketchResult = { plane: PlaneFrame; solve: SolveResult; profiles: { outer: Loop; holes: Loop[] }[]; curves: SketchCurve[] };
export type Summary = { name: string; features: FeatureSummary[]; bodies: BodySummary[]; sketches: Record<string, SketchResult>; variables: Record<string, number> };

export type SketchOp =
  | { type: "add_point"; pos: Vec2 }
  | { type: "add_line"; a: Vec2; b: Vec2 }
  | { type: "add_rectangle"; a: Vec2; b: Vec2 }
  | { type: "add_circle"; center: Vec2; radius: number }
  | { type: "add_arc"; center: Vec2; start: Vec2; end: Vec2 }
  | { type: "add_constraint"; constraint: Constraint }
  | { type: "remove_constraint"; id: number }
  | { type: "remove_entity"; id: number }
  | { type: "set_constraint_value"; id: number; value: number }
  | { type: "move_point"; id: number; pos: Vec2 }
  | { type: "set_construction"; id: number; construction: boolean };

export type Op =
  | { type: "add_sketch"; plane: PlaneRef; name: string | null }
  | { type: "add_extrude"; sketch: number; depth: number; direction?: ExtrudeDirection; end?: ExtrudeEnd; profiles?: ProfileSelection; op?: BodyOp; name: string | null }
  | { type: "set_extrude"; id: number; depth?: number | null; direction?: ExtrudeDirection | null; end?: ExtrudeEnd | null; profiles?: ProfileSelection | null; op?: BodyOp | null }
  | { type: "add_revolve"; sketch: number; axis: RevolveAxis; angle?: number; profiles?: ProfileSelection; op?: BodyOp; name: string | null }
  | { type: "set_revolve"; id: number; axis?: RevolveAxis | null; angle?: number | null; profiles?: ProfileSelection | null; op?: BodyOp | null }
  | { type: "add_blend"; kind: BlendKind; edges: EdgeRef[]; size: number; name: string | null }
  | { type: "set_blend"; id: number; edges?: EdgeRef[] | null; size?: number | null }
  | { type: "add_mirror"; plane: PlaneRef; op?: CopyOp; name: string | null }
  | { type: "set_mirror"; id: number; plane?: PlaneRef | null; op?: CopyOp | null }
  | { type: "add_pattern"; kind: PatternKind; count: number; op?: CopyOp; name: string | null }
  | { type: "set_pattern"; id: number; kind?: PatternKind | null; count?: number | null; op?: CopyOp | null }
  | { type: "add_variable"; name: string; expression: string }
  | { type: "set_variable"; id: number; name?: string | null; expression?: string | null }
  | { type: "set_binding"; id: number; field: string; expression: string | null }
  | { type: "set_sketch_plane"; id: number; plane: PlaneRef }
  | { type: "rename_feature"; id: number; name: string }
  | { type: "set_suppressed"; id: number; suppressed: boolean }
  | { type: "delete_feature"; id: number }
  | { type: "move_feature"; id: number; index: number }
  | { type: "sketch"; id: number; op: SketchOp }
  | { type: "rename_studio"; name: string };

export type OpResult = { feature: number | null; entities: number[]; constraint: number | null };

export type BodyMesh = { positions: Float32Array; normals: Float32Array; indices: Uint32Array; edges: Float32Array; edgeFaces: Uint32Array; faceIds: Uint32Array };

export class Kernel {
  private studio: Studio;

  private constructor(studio: Studio) {
    this.studio = studio;
  }

  static async load(): Promise<typeof Kernel> {
    await init();
    return Kernel;
  }

  static empty(): Kernel {
    return new Kernel(new Studio());
  }

  static demo(): Kernel {
    return new Kernel(Studio.demo());
  }

  static fromJson(json: string): Kernel {
    return new Kernel(Studio.from_json(json));
  }

  static version(): string {
    return kernelVersion();
  }

  toJson(): string {
    return this.studio.to_json();
  }

  apply(op: Op): OpResult {
    return JSON.parse(this.studio.apply(JSON.stringify(op))) as OpResult;
  }

  /** Regenerates; with `rollback`, the result reflects the state after that many features. */
  regenerate(rollback: number | null = null): Summary {
    return JSON.parse(this.studio.regenerate(rollback ?? undefined)) as Summary;
  }

  bodyMeshes(): BodyMesh[] {
    const out: BodyMesh[] = [];
    for (let i = 0; i < this.studio.body_count(); i++) {
      out.push({
        positions: this.studio.body_positions(i),
        normals: this.studio.body_normals(i),
        indices: this.studio.body_indices(i),
        edges: this.studio.body_edges(i),
        edgeFaces: this.studio.body_edge_faces(i),
        faceIds: this.studio.body_face_ids(i),
      });
    }
    return out;
  }

  dispose(): void {
    this.studio.free();
  }
}
