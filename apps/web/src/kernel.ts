// Typed wrapper around the WebAssembly kernel. All document edits are
// `DocOp` values (see crates/ok-model/src/document.rs) sent as JSON: an
// `Op` on a part studio tab, an `AssemblyOp` on an assembly tab, or a
// tab-level change.

import init, { Doc, version as kernelVersion } from "./wasm/ok_wasm.js";

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
  | { type: "tangent"; line: number; entity: number }
  | { type: "symmetric"; a: number; b: number; line: number };

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
  /** Ids of entities projected from body geometry (absent when none). */
  projected?: number[];
};

/** Body geometry mirrored into a sketch ("Use"). */
export type ProjectionSource = { type: "edge"; edge: EdgeRef } | { type: "face"; face: FaceRef };
export type Projection = { source: ProjectionSource; block: number; entities: number[] };

export type RevolveAxis = { type: "x_axis" } | { type: "y_axis" } | { type: "line"; line: number };
/** An edge of a body, named by the two faces that meet there. */
export type EdgeRef = { a: FaceRef; b: FaceRef };
export type BlendKind = "fillet" | "chamfer";
export type CopyOp = "add" | "new";
export type Axis = "x" | "y" | "z";
export type PatternKind = { type: "linear"; axis: Axis; spacing: number } | { type: "circular"; axis: Axis; angle: number };
export type FeatureKind =
  | { type: "sketch"; plane: PlaneRef; sketch: SketchData; projections?: Projection[] }
  | { type: "extrude"; sketch: number; profiles: ProfileSelection; depth: number; direction: ExtrudeDirection; end: ExtrudeEnd; op: BodyOp }
  | { type: "revolve"; sketch: number; profiles: ProfileSelection; axis: RevolveAxis; angle: number; op: BodyOp }
  | { type: "blend"; kind: BlendKind; edges: EdgeRef[]; size: number }
  | { type: "mirror"; plane: PlaneRef; op: CopyOp }
  | { type: "pattern"; kind: PatternKind; count: number; op: CopyOp }
  | { type: "variable"; name: string; expression: string }
  | { type: "hole"; sketch: number; diameter: number; depth: number; through_all: boolean; direction: ExtrudeDirection; counterbore: Counterbore | null }
  | { type: "sweep"; sketch: number; profiles: ProfileSelection; path: number; op: BodyOp }
  | { type: "loft"; sketch: number; sketch_b: number; op: BodyOp };
export type Counterbore = { diameter: number; depth: number };

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
  area: number;
  centroid: Vec3 | null;
};
export type SolveResult = {
  status: "fully_constrained" | "under_constrained" | "inconsistent";
  iterations: number;
  max_residual: number;
  dof: number;
  equations: number;
  parameters: number;
};
export type SketchCurve = { entity: number; kind: string; construction: boolean; projected: boolean; points: Vec3[] };
export type PlaneFrame = { origin: Vec3; x_axis: Vec3; y_axis: Vec3; normal: Vec3 };
export type Loop = { points: Vec2[] };
export type SketchResult = { plane: PlaneFrame; solve: SolveResult; profiles: { outer: Loop; holes: Loop[] }[]; curves: SketchCurve[] };
export type Settings = { facet_angle: number };
export type TabKind = "part_studio" | "assembly";
export type TabSummary = { id: number; name: string; kind: TabKind };

// ---- assemblies
export type Placement = { position: Vec3; rotation: Vec3 };
export type MateKind = "fastened" | "revolute" | "slider" | "cylindrical";
/** A face of an instance's body, used as a mate connector. */
export type Connector = { instance: number; face: FaceRef };
export type Transform = { m: number[][]; t: Vec3 };
export type InstanceSummary = {
  id: number;
  name: string;
  studio: number;
  body: number;
  fixed: boolean;
  placement: Placement;
  /** Index into `bodies` when the instance resolved. */
  body_index: number | null;
  transform: Transform | null;
  error: string | null;
};
export type MateSummary = {
  id: number;
  name: string;
  kind: MateKind;
  a: Connector;
  b: Connector;
  offset: number;
  angle: number;
  flip: boolean;
  error: string | null;
};

export type Summary = {
  /** Document name. */
  name: string;
  tabs: TabSummary[];
  /** The regenerated tab. */
  tab: number;
  kind: TabKind;
  tab_name: string;
  features: FeatureSummary[];
  bodies: BodySummary[];
  sketches: Record<string, SketchResult>;
  variables: Record<string, number>;
  settings: Settings;
  instances: InstanceSummary[];
  mates: MateSummary[];
};

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
  | { type: "set_construction"; id: number; construction: boolean }
  | { type: "trim"; entity: number; at: Vec2 }
  | { type: "offset"; entities: number[]; distance: number }
  | { type: "mirror"; entities: number[]; axis: number }
  | { type: "project"; source: ProjectionSource }
  | { type: "remove_projection"; index: number }
  | { type: "restore"; [key: string]: unknown };

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
  | { type: "add_hole"; sketch: number; diameter: number; depth?: number; through_all?: boolean; direction?: ExtrudeDirection; counterbore?: Counterbore | null; name: string | null }
  | { type: "set_hole"; id: number; diameter?: number | null; depth?: number | null; through_all?: boolean | null; direction?: ExtrudeDirection | null; counterbore?: Counterbore | null }
  | { type: "add_sweep"; sketch: number; path: number; profiles?: ProfileSelection; op?: BodyOp; name: string | null }
  | { type: "set_sweep"; id: number; path?: number | null; profiles?: ProfileSelection | null; op?: BodyOp | null }
  | { type: "add_loft"; sketch: number; sketch_b: number; op?: BodyOp; name: string | null }
  | { type: "set_loft"; id: number; sketch_b?: number | null; op?: BodyOp | null }
  | { type: "set_settings"; facet_angle: number }
  | { type: "replace_document"; json: string }
  /** Inverse ops (undo) carry saved state; the client never builds these itself. */
  | { type: "insert_feature"; index: number; feature: unknown }
  | { type: "set_sketch_plane"; id: number; plane: PlaneRef }
  | { type: "rename_feature"; id: number; name: string }
  | { type: "set_suppressed"; id: number; suppressed: boolean }
  | { type: "delete_feature"; id: number }
  | { type: "move_feature"; id: number; index: number }
  | { type: "sketch"; id: number; op: SketchOp }
  | { type: "rename_studio"; name: string };

/** Ops that undo an applied op come back with its result (see `inverse`). */
export type OpResult = { feature: number | null; entities: number[]; constraint: number | null; inverse?: Op[] };

export type AssemblyOp =
  | { type: "add_instance"; studio: number; body: number; name: string | null; fixed?: boolean; placement?: Placement }
  | { type: "remove_instance"; id: number }
  | { type: "set_instance"; id: number; name?: string | null; fixed?: boolean | null; placement?: Placement | null }
  | { type: "add_mate"; kind?: MateKind; a: Connector; b: Connector; offset?: number; angle?: number; flip?: boolean; name: string | null }
  | { type: "set_mate"; id: number; name?: string | null; kind?: MateKind | null; a?: Connector | null; b?: Connector | null; offset?: number | null; angle?: number | null; flip?: boolean | null }
  | { type: "remove_mate"; id: number }
  | { type: "restore"; instances: unknown[]; mates: unknown[] };

/** An edit to the document: a studio op on a tab, an assembly op, or a tab change. */
export type DocOp =
  | { type: "add_part_studio"; name: string | null }
  | { type: "add_assembly"; name: string | null }
  | { type: "rename_tab"; tab: number; name: string }
  | { type: "delete_tab"; tab: number }
  | { type: "insert_tab"; index: number; tab: unknown }
  | { type: "rename_document"; name: string }
  | { type: "studio"; tab: number; op: Op }
  | { type: "assembly"; tab: number; op: AssemblyOp }
  | { type: "replace_document"; json: string };

export type DocOpResult = { tab: number | null; instance: number | null; mate: number | null; studio?: OpResult; inverse?: DocOp[] };

export type BodyMesh = { positions: Float32Array; normals: Float32Array; indices: Uint32Array; edges: Float32Array; edgeFaces: Uint32Array; faceIds: Uint32Array; faceSurfaces: Uint32Array };

export class Kernel {
  private studio: Doc;

  private constructor(studio: Doc) {
    this.studio = studio;
  }

  static async load(): Promise<typeof Kernel> {
    await init();
    return Kernel;
  }

  static empty(): Kernel {
    return new Kernel(new Doc());
  }

  static demo(): Kernel {
    return new Kernel(Doc.demo());
  }

  static fromJson(json: string): Kernel {
    return new Kernel(Doc.from_json(json));
  }

  static version(): string {
    return kernelVersion();
  }

  toJson(): string {
    return this.studio.to_json();
  }

  /** Applies a document op; `base` makes new ids start there (collaborative id ranges). */
  apply(op: DocOp, base: number | null = null): DocOpResult {
    return JSON.parse(this.studio.apply(JSON.stringify(op), base ?? undefined)) as DocOpResult;
  }

  /** The first part studio tab, if any. */
  firstStudio(): number | null {
    return this.studio.first_studio() ?? null;
  }

  /** Names of the bodies a part studio tab produces. */
  studioBodies(tab: number): string[] {
    return JSON.parse(this.studio.studio_bodies(tab)) as string[];
  }

  /** Structural hash (hex) for replica consistency checks. */
  structuralHash(): string {
    return this.studio.structural_hash();
  }

  /** Regenerates a tab (the first part studio when `tab` is null); with `rollback`, a studio reflects the state after that many features. */
  regenerate(tab: number | null = null, rollback: number | null = null): Summary {
    return JSON.parse(this.studio.regenerate(tab ?? undefined, rollback ?? undefined)) as Summary;
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
        faceSurfaces: this.studio.body_face_surfaces(i),
      });
    }
    return out;
  }

  dispose(): void {
    this.studio.free();
  }
}
