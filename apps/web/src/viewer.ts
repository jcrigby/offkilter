// three.js viewport: bodies, sketch curves, grid and standard planes.

import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { CSS2DObject, CSS2DRenderer } from "three/examples/jsm/renderers/CSS2DRenderer.js";
import type { BodyMesh, PlaneFrame, RigidTransform, SketchResult, Vec2, Vec3 } from "./kernel";

const BODY_COLOR = 0x8fa8c8;
const SKETCH_COLOR = 0x4ea1ff;
const SKETCH_SELECTED = 0xffd166;
const FACE_SELECTED = 0xffd166;
const FACE_HOVER = 0x4ea1ff;

/** A picked face: body index and face index within that body's solid. */
export type FacePick = { body: number; face: number };
/** A picked edge: body index and the two face indices it separates. */
export type EdgePick = { body: number; faces: [number, number]; segment: number };

/** A clickable text label anchored to a world position. */
export type Label = { position: Vec3; text: string; className?: string; onClick?: () => void; title?: string };

/** Pointer events routed to a sketch tool while sketch mode is active. */
export interface PointerHandler {
  down(e: PointerEvent): void;
  move(e: PointerEvent): void;
  up(e: PointerEvent): void;
  cancel(): void;
}

const SKETCH_ENTITY_SELECTED = 0xff7b3a;
const SKETCH_PROJECTED = 0xb48cff;

export class Viewer {
  private renderer: THREE.WebGLRenderer;
  private scene = new THREE.Scene();
  private camera: THREE.PerspectiveCamera;
  private controls: OrbitControls;
  private bodies = new THREE.Group();
  private sketches = new THREE.Group();
  // Normals come from the kernel (analytic on curved surfaces), so no flat shading.
  private bodyMaterial = new THREE.MeshStandardMaterial({ color: BODY_COLOR, metalness: 0.1, roughness: 0.6 });
  private selectedBodyMaterial = new THREE.MeshStandardMaterial({ color: FACE_SELECTED, metalness: 0.1, roughness: 0.6 });
  private edgeMaterial = new THREE.LineBasicMaterial({ color: 0x1b1d21 });
  private meshes: THREE.Mesh[] = [];
  private meshData: BodyMesh[] = [];
  private highlight = new THREE.Group();
  private hover = new THREE.Group();
  private raycaster = new THREE.Raycaster();
  private pointerDown: { x: number; y: number } | null = null;
  /** Called when the user clicks a face (or empty space with `null`). */
  onPick: ((pick: FacePick | null, e?: PointerEvent) => void) | null = null;
  /** In `pickMode`, also hover edges (mate connectors sit on faces, edges or corners). */
  pickConnectors = false;
  /** Called instead of `onPick` while `edgePickMode` is on. */
  onEdgePick: ((pick: EdgePick | null) => void) | null = null;
  /** When true, hovering previews faces; used while a panel waits for a face. */
  pickMode = false;
  /** When true, clicks pick edges rather than faces. */
  edgePickMode = false;
  private edgeLines: THREE.LineSegments[] = [];
  private edgeHighlight = new THREE.Group();
  private edgeHover = new THREE.Group();
  /** Receives left-button pointer events instead of picking while set. */
  pointerHandler: PointerHandler | null = null;
  private preview = new THREE.Group();
  /** Mate connector frames drawn as axis triads. */
  private frames = new THREE.Group();
  private previewMaterial = new THREE.LineBasicMaterial({ color: SKETCH_SELECTED, depthTest: false });
  private labelRenderer = new CSS2DRenderer();
  private labels = new THREE.Group();
  /** Measurement overlay: two points, a line and a label. */
  private measure = new THREE.Group();
  /** Body indices hidden by the user; they are neither drawn nor picked. */
  private hidden = new Set<number>();
  /** Explode factor: bodies slide away from the common centre by this fraction of their distance. */
  private explode = 0;
  private offsets: Vec3[] = [];

  constructor(private container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.renderer.setClearColor(0x1b1d21);
    container.appendChild(this.renderer.domElement);
    this.labelRenderer.domElement.className = "labels";
    container.appendChild(this.labelRenderer.domElement);

    // CAD convention: Z up.
    this.camera = new THREE.PerspectiveCamera(40, 1, 0.1, 10000);
    this.camera.up.set(0, 0, 1);
    this.camera.position.set(120, -140, 100);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.target.set(30, 20, 5);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.12;

    const hemi = new THREE.HemisphereLight(0xffffff, 0x334455, 0.9);
    this.scene.add(hemi);
    const key = new THREE.DirectionalLight(0xffffff, 1.2);
    key.position.set(100, -80, 150);
    this.scene.add(key);
    const fill = new THREE.DirectionalLight(0xffffff, 0.4);
    fill.position.set(-100, 60, -40);
    this.scene.add(fill);

    const grid = new THREE.GridHelper(400, 40, 0x3a3f48, 0x2a2e35);
    grid.rotation.x = Math.PI / 2; // GridHelper is XZ by default; rotate into XY
    this.scene.add(grid);
    const axes = new THREE.AxesHelper(20);
    this.scene.add(axes);

    this.scene.add(this.bodies);
    this.scene.add(this.sketches);
    this.scene.add(this.highlight);
    this.scene.add(this.hover);
    this.scene.add(this.edgeHighlight);
    this.scene.add(this.edgeHover);

    this.scene.add(this.preview);
    this.scene.add(this.frames);
    this.scene.add(this.labels);
    this.scene.add(this.measure);
    this.renderer.localClippingEnabled = true;
    const el = this.renderer.domElement;
    el.addEventListener("contextmenu", (e) => e.preventDefault());
    el.addEventListener("pointerdown", (e) => {
      if (this.pointerHandler && e.button === 0) {
        this.pointerHandler.down(e);
        return;
      }
      this.pointerDown = { x: e.clientX, y: e.clientY };
    });
    el.addEventListener("pointerup", (e) => {
      if (this.pointerHandler && e.button === 0) {
        this.pointerHandler.up(e);
        return;
      }
      if (this.pointerHandler && e.button === 2) {
        this.pointerHandler.cancel();
      }
      const down = this.pointerDown;
      this.pointerDown = null;
      if (!down || e.button !== 0) return;
      if (Math.hypot(e.clientX - down.x, e.clientY - down.y) > 4) return; // it was a drag
      if (this.edgePickMode) this.onEdgePick?.(this.pickEdgeAt(e));
      else this.onPick?.(this.pickAt(e), e);
    });
    el.addEventListener("pointermove", (e) => {
      if (this.pointerHandler) {
        this.pointerHandler.move(e);
        return;
      }
      if (this.pointerDown) return;
      if (this.edgePickMode) this.showEdges(this.edgeHover, this.pickEdgeAt(e) ? [this.pickEdgeAt(e)!] : [], FACE_HOVER);
      else if (this.pickMode && this.pickConnectors) this.hoverEdgeOrFace(e);
      else if (this.pickMode) this.showFace(this.hover, this.pickAt(e), FACE_HOVER, 0.35);
    });
    el.addEventListener("pointerleave", () => this.clear(this.hover));

    new ResizeObserver(() => this.resize()).observe(container);
    this.resize();
    this.animate();
  }

  private resize(): void {
    const w = this.container.clientWidth || 1;
    const h = this.container.clientHeight || 1;
    this.renderer.setSize(w, h, false);
    this.labelRenderer.setSize(w, h);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }

  private animate = (): void => {
    requestAnimationFrame(this.animate);
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
    this.labelRenderer.render(this.scene, this.camera);
  };

  setBodies(meshes: BodyMesh[]): void {
    this.clear(this.bodies);
    this.clear(this.highlight);
    this.clear(this.hover);
    this.clear(this.edgeHighlight);
    this.clear(this.edgeHover);
    this.meshes = [];
    this.edgeLines = [];
    this.meshData = meshes;
    for (const m of meshes) {
      const geom = new THREE.BufferGeometry();
      geom.setAttribute("position", new THREE.BufferAttribute(m.positions, 3));
      geom.setAttribute("normal", new THREE.BufferAttribute(m.normals, 3));
      geom.setIndex(new THREE.BufferAttribute(m.indices, 1));
      const mesh = new THREE.Mesh(geom, this.bodyMaterial);
      mesh.userData.body = this.meshes.length;
      mesh.visible = !this.hidden.has(this.meshes.length);
      this.meshes.push(mesh);
      this.bodies.add(mesh);
      if (m.edges.length > 0) {
        const eg = new THREE.BufferGeometry();
        eg.setAttribute("position", new THREE.BufferAttribute(m.edges, 3));
        const lines = new THREE.LineSegments(eg, this.edgeMaterial);
        lines.userData.body = this.meshes.length - 1;
        lines.visible = mesh.visible;
        this.edgeLines.push(lines);
        this.bodies.add(lines);
      }
    }
    if (this.explode !== 0) this.setExplode(this.explode);
    else this.offsets = meshes.map(() => ({ x: 0, y: 0, z: 0 }));
  }

  /** Slides every body away from the bodies' common centre (0 = in place). */
  setExplode(factor: number): void {
    this.explode = factor;
    const centres = this.meshes.map((m) => {
      m.geometry.computeBoundingBox();
      return m.geometry.boundingBox!.getCenter(new THREE.Vector3());
    });
    if (centres.length === 0) return;
    const all = centres.reduce((a, c) => a.add(c), new THREE.Vector3()).divideScalar(centres.length);
    this.offsets = centres.map((c) => {
      const d = c.clone().sub(all).multiplyScalar(factor);
      return { x: d.x, y: d.y, z: d.z };
    });
    for (const m of this.meshes) {
      const o = this.offsets[m.userData.body as number]!;
      m.position.set(o.x, o.y, o.z);
    }
    for (const l of this.edgeLines) {
      const o = this.offsets[l.userData.body as number]!;
      l.position.set(o.x, o.y, o.z);
    }
  }

  /**
   * Moves bodies by rigid transforms on top of their placement (a mate animation frame); `null` entries
   * and a `null` list leave bodies where the kernel put them. Explode offsets still apply.
   */
  setBodyTransforms(deltas: (RigidTransform | null)[] | null): void {
    for (const obj of [...this.meshes, ...this.edgeLines]) {
      const i = obj.userData.body as number;
      const d = deltas?.[i] ?? null;
      const o = this.offsets[i] ?? { x: 0, y: 0, z: 0 };
      if (!d) {
        obj.matrixAutoUpdate = true;
        obj.rotation.set(0, 0, 0);
        obj.position.set(o.x, o.y, o.z);
        obj.updateMatrix();
        continue;
      }
      obj.matrixAutoUpdate = false;
      obj.matrix.set(d.m[0][0], d.m[0][1], d.m[0][2], d.t.x + o.x, d.m[1][0], d.m[1][1], d.m[1][2], d.t.y + o.y, d.m[2][0], d.m[2][1], d.m[2][2], d.t.z + o.z, 0, 0, 0, 1);
      obj.matrixWorldNeedsUpdate = true;
    }
  }

  /** The matrix currently placing a body (for tests): identity unless a transform or explode offset is set. */
  bodyMatrix(i: number): number[] {
    const m = this.meshes[i];
    if (!m) return [];
    m.updateMatrixWorld(true);
    return m.matrixWorld.toArray();
  }

  /** Current explode displacement of a body (for tests and labels). */
  bodyOffset(index: number): Vec3 {
    return this.offsets[index] ?? { x: 0, y: 0, z: 0 };
  }

  /** Index of the body under the pointer, if any. */
  pickBodyIndex(e: PointerEvent): number | null {
    return this.pickAt(e)?.body ?? null;
  }

  /** Where the pointer ray meets the plane through `origin` facing the camera. */
  pickOnViewPlane(e: PointerEvent, origin: Vec3): Vec3 | null {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.camera);
    const n = this.camera.getWorldDirection(new THREE.Vector3());
    const plane = new THREE.Plane().setFromNormalAndCoplanarPoint(n, new THREE.Vector3(origin.x, origin.y, origin.z));
    const hit = new THREE.Vector3();
    if (!this.raycaster.ray.intersectPlane(plane, hit)) return null;
    return { x: hit.x, y: hit.y, z: hit.z };
  }

  /** Shows or hides one body (by index) without touching the document. */
  setBodyVisible(index: number, visible: boolean): void {
    if (visible) this.hidden.delete(index);
    else this.hidden.add(index);
    for (const m of this.meshes) if (m.userData.body === index) m.visible = visible;
    for (const l of this.edgeLines) if (l.userData.body === index) l.visible = visible;
  }

  /** Body indices currently hidden. */
  hiddenBodies(): ReadonlySet<number> {
    return this.hidden;
  }

  /** Forgets all hidden bodies (e.g. when switching tabs). */
  showAllBodies(): void {
    for (const i of [...this.hidden]) this.setBodyVisible(i, true);
  }

  /** Moves the camera to a standard orthographic-style direction, keeping the target and distance. */
  setStandardView(view: "top" | "front" | "right" | "iso"): void {
    const target = this.controls.target.clone();
    const dist = this.camera.position.distanceTo(target);
    const dir =
      view === "top" ? new THREE.Vector3(0, 0, 1) : view === "front" ? new THREE.Vector3(0, -1, 0) : view === "right" ? new THREE.Vector3(1, 0, 0) : new THREE.Vector3(0.6, -0.7, 0.5).normalize();
    this.camera.up.set(0, view === "top" ? 1 : 0, view === "top" ? 0 : 1);
    this.camera.position.copy(target).addScaledVector(dir, dist);
    this.camera.lookAt(target);
    this.controls.update();
  }

  /**
   * Clips everything on one side of an axis-aligned plane so the inside of
   * bodies can be seen; `null` turns the section off.
   */
  setSection(section: { axis: "x" | "y" | "z"; offset: number; flip: boolean } | null): void {
    const planes: THREE.Plane[] = [];
    if (section) {
      const n = new THREE.Vector3(section.axis === "x" ? 1 : 0, section.axis === "y" ? 1 : 0, section.axis === "z" ? 1 : 0);
      if (section.flip) n.negate();
      // Keep the half-space n·p <= offset·|n| side of the plane (three.js keeps n·p + c >= 0).
      const sign = section.flip ? 1 : -1;
      planes.push(new THREE.Plane(n, sign * section.offset));
    }
    for (const m of [this.bodyMaterial, this.selectedBodyMaterial, this.edgeMaterial]) {
      m.clippingPlanes = planes;
      m.needsUpdate = true;
    }
    this.bodyMaterial.side = section ? THREE.DoubleSide : THREE.FrontSide;
    this.selectedBodyMaterial.side = section ? THREE.DoubleSide : THREE.FrontSide;
  }

  /** Unit vector the camera looks along. */
  viewDirection(): Vec3 {
    const d = this.controls.target.clone().sub(this.camera.position).normalize();
    return { x: d.x, y: d.y, z: d.z };
  }

  /** Bounding box of the shown bodies, or null when there are none. */
  bodyBounds(): { min: Vec3; max: Vec3 } | null {
    const box = new THREE.Box3();
    for (const m of this.meshes) if (m.visible) box.expandByObject(m);
    if (box.isEmpty()) return null;
    return { min: { x: box.min.x, y: box.min.y, z: box.min.z }, max: { x: box.max.x, y: box.max.y, z: box.max.z } };
  }

  /** Point on a body under the pointer, snapped to the nearest mesh vertex when within a few pixels. */
  pickPoint(e: PointerEvent): { point: Vec3; body: number; face: number; snapped: boolean } | null {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.camera);
    const hit = this.raycaster.intersectObjects(this.meshes.filter((m) => m.visible), false)[0];
    if (!hit || hit.faceIndex === undefined || hit.faceIndex === null || !hit.face) return null;
    const body = hit.object.userData.body as number;
    const data = this.meshData[body];
    if (!data) return null;
    const face = data.faceIds[hit.faceIndex];
    if (face === undefined) return null;
    let point: THREE.Vector3 = hit.point.clone();
    let snapped = false;
    // Snap to the closest corner of the hit triangle when it is within 8 screen pixels.
    const px = this.eventPx(e);
    for (const vi of [hit.face.a, hit.face.b, hit.face.c]) {
      const v = new THREE.Vector3(data.positions[3 * vi]!, data.positions[3 * vi + 1]!, data.positions[3 * vi + 2]!);
      const s = this.toScreen({ x: v.x, y: v.y, z: v.z });
      if (Math.hypot(s.x - px.x, s.y - px.y) < 8) {
        point = v;
        snapped = true;
        break;
      }
    }
    return { point: { x: point.x, y: point.y, z: point.z }, body, face, snapped };
  }

  /** The body corner within 8 screen pixels of the pointer, with every face that meets there. */
  pickVertex(e: PointerEvent): { body: number; point: Vec3; faces: number[] } | null {
    const hit = this.pickPoint(e);
    if (!hit || !hit.snapped) return null;
    const data = this.meshData[hit.body];
    if (!data) return null;
    const faces = new Set<number>();
    const p = hit.point;
    for (let t = 0; t < data.faceIds.length; t++) {
      for (let k = 0; k < 3; k++) {
        const vi = data.indices[3 * t + k]!;
        const d = Math.abs(data.positions[3 * vi]! - p.x) + Math.abs(data.positions[3 * vi + 1]! - p.y) + Math.abs(data.positions[3 * vi + 2]! - p.z);
        if (d < 1e-6) {
          faces.add(data.faceIds[t]!);
          break;
        }
      }
    }
    return { body: hit.body, point: p, faces: [...faces] };
  }

  /** Draws a measurement between two points with a label; `null` clears it. */
  setMeasure(m: { a: Vec3; b: Vec3 | null; text: string } | null): void {
    this.clear(this.measure);
    if (!m) return;
    const mk = (p: Vec3): THREE.Mesh => {
      const s = new THREE.Mesh(new THREE.SphereGeometry(0.6, 12, 12), new THREE.MeshBasicMaterial({ color: SKETCH_SELECTED, depthTest: false }));
      s.position.set(p.x, p.y, p.z);
      s.renderOrder = 10;
      return s;
    };
    this.measure.add(mk(m.a));
    if (m.b) {
      this.measure.add(mk(m.b));
      const g = new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(m.a.x, m.a.y, m.a.z), new THREE.Vector3(m.b.x, m.b.y, m.b.z)]);
      const line = new THREE.Line(g, this.previewMaterial);
      line.renderOrder = 10;
      this.measure.add(line);
    }
    const div = document.createElement("div");
    div.className = "label measure";
    div.textContent = m.text;
    const obj = new CSS2DObject(div);
    const mid = m.b ? { x: (m.a.x + m.b.x) / 2, y: (m.a.y + m.b.y) / 2, z: (m.a.z + m.b.z) / 2 } : m.a;
    obj.position.set(mid.x, mid.y, mid.z);
    this.measure.add(obj);
  }

  setSketches(sketches: Record<string, SketchResult>, selected: number | null, selectedEntities: ReadonlySet<number> = new Set()): void {
    this.clear(this.sketches);
    for (const [id, sk] of Object.entries(sketches)) {
      const isSelected = Number(id) === selected;
      const color = isSelected ? SKETCH_SELECTED : SKETCH_COLOR;
      const lineMat = new THREE.LineBasicMaterial({ color, depthTest: !isSelected });
      const pointMat = new THREE.PointsMaterial({ color, size: isSelected ? 7 : 4, sizeAttenuation: false, depthTest: !isSelected });
      const projLineMat = new THREE.LineBasicMaterial({ color: SKETCH_PROJECTED, depthTest: !isSelected });
      const projPointMat = new THREE.PointsMaterial({ color: SKETCH_PROJECTED, size: isSelected ? 7 : 4, sizeAttenuation: false, depthTest: !isSelected });
      const selLineMat = new THREE.LineBasicMaterial({ color: SKETCH_ENTITY_SELECTED, depthTest: false, linewidth: 2 });
      const selPointMat = new THREE.PointsMaterial({ color: SKETCH_ENTITY_SELECTED, size: 10, sizeAttenuation: false, depthTest: false });
      const dashMat = new THREE.LineDashedMaterial({ color, depthTest: !isSelected, dashSize: 1.5, gapSize: 1 });
      const selDashMat = new THREE.LineDashedMaterial({ color: SKETCH_ENTITY_SELECTED, depthTest: false, dashSize: 1.5, gapSize: 1 });
      for (const c of sk.curves) {
        const pts = c.points.map((p: Vec3) => new THREE.Vector3(p.x, p.y, p.z));
        const sel = isSelected && selectedEntities.has(c.entity);
        if (c.kind === "point") {
          this.sketches.add(new THREE.Points(new THREE.BufferGeometry().setFromPoints(pts), sel ? selPointMat : c.projected ? projPointMat : pointMat));
        } else if (c.projected && !c.construction) {
          this.sketches.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), sel ? selLineMat : projLineMat));
        } else if (c.construction) {
          const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), sel ? selDashMat : dashMat);
          line.computeLineDistances();
          this.sketches.add(line);
        } else {
          this.sketches.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), sel ? selLineMat : lineMat));
        }
      }
    }
  }

  /** Replaces the clickable labels shown over the scene. */
  setLabels(labels: Label[]): void {
    this.clear(this.labels);
    for (const l of labels) {
      const div = document.createElement("div");
      div.className = "label " + (l.className ?? "");
      div.textContent = l.text;
      if (l.title) div.title = l.title;
      if (l.onClick) {
        div.classList.add("clickable");
        div.onpointerdown = (e) => e.stopPropagation();
        div.onclick = (e) => {
          e.stopPropagation();
          l.onClick?.();
        };
      }
      const obj = new CSS2DObject(div);
      obj.position.set(l.position.x, l.position.y, l.position.z);
      this.labels.add(obj);
    }
  }

  /** Temporary geometry drawn while a tool is in progress (rubber band). */
  setPreview(polylines: Vec3[][]): void {
    this.clear(this.preview);
    for (const pl of polylines) {
      const pts = pl.map((p) => new THREE.Vector3(p.x, p.y, p.z));
      this.preview.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), this.previewMaterial));
    }
  }

  /** Intersects the pointer ray with a plane; returns plane coordinates. */
  toPlane(e: PointerEvent, plane: PlaneFrame): Vec2 | null {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.camera);
    const n = new THREE.Vector3(plane.normal.x, plane.normal.y, plane.normal.z);
    const o = new THREE.Vector3(plane.origin.x, plane.origin.y, plane.origin.z);
    const p3 = new THREE.Plane().setFromNormalAndCoplanarPoint(n, o);
    const hit = new THREE.Vector3();
    if (!this.raycaster.ray.intersectPlane(p3, hit)) return null;
    const d = hit.sub(o);
    return { x: d.dot(new THREE.Vector3(plane.x_axis.x, plane.x_axis.y, plane.x_axis.z)), y: d.dot(new THREE.Vector3(plane.y_axis.x, plane.y_axis.y, plane.y_axis.z)) };
  }

  /** Projects a world point to pixel coordinates within the viewport. */
  toScreen(p: Vec3): { x: number; y: number } {
    const v = new THREE.Vector3(p.x, p.y, p.z).project(this.camera);
    const rect = this.renderer.domElement.getBoundingClientRect();
    return { x: ((v.x + 1) / 2) * rect.width, y: ((1 - v.y) / 2) * rect.height };
  }

  /** Pixel coordinates of a pointer event within the viewport. */
  eventPx(e: PointerEvent): { x: number; y: number } {
    const rect = this.renderer.domElement.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  }

  /** Orients the camera to look straight down a plane's normal. */
  lookAtPlane(plane: PlaneFrame): void {
    const target = this.controls.target.clone();
    const dist = this.camera.position.distanceTo(target);
    const n = new THREE.Vector3(plane.normal.x, plane.normal.y, plane.normal.z);
    this.camera.position.copy(target).addScaledVector(n, dist);
    this.camera.up.set(plane.y_axis.x, plane.y_axis.y, plane.y_axis.z);
    this.camera.lookAt(target);
    this.controls.update();
  }

  /** Restores the default Z-up camera. */
  resetUp(): void {
    this.camera.up.set(0, 0, 1);
    this.controls.update();
  }

  /** Left button draws in sketch mode; rotate moves to the right button. */
  setSketchMouse(on: boolean): void {
    this.controls.mouseButtons = on
      ? { LEFT: null as unknown as THREE.MOUSE, MIDDLE: THREE.MOUSE.PAN, RIGHT: THREE.MOUSE.ROTATE }
      : { LEFT: THREE.MOUSE.ROTATE, MIDDLE: THREE.MOUSE.DOLLY, RIGHT: THREE.MOUSE.PAN };
  }

  /** The edge under the pointer if one is within a few pixels, else the face. */
  pickEdgeOrFace(e: PointerEvent): { edge: EdgePick } | { face: FacePick } | null {
    const edge = this.pickEdgeAt(e);
    if (edge) return { edge };
    const face = this.pickAt(e);
    return face ? { face } : null;
  }

  /** Hover preview for `pickEdgeOrFace` (used by the sketch "Use" tool). */
  hoverEdgeOrFace(e: PointerEvent | null): void {
    const pick = e ? this.pickEdgeOrFace(e) : null;
    this.showEdges(this.edgeHover, pick && "edge" in pick ? [pick.edge] : [], FACE_HOVER);
    this.showFace(this.hover, pick && "face" in pick ? pick.face : null, FACE_HOVER, 0.35);
  }

  /** Face under a pointer event, if any. */
  private pickAt(e: PointerEvent): FacePick | null {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.camera);
    const hit = this.raycaster.intersectObjects(this.meshes.filter((m) => m.visible), false)[0];
    if (!hit || hit.faceIndex === undefined || hit.faceIndex === null) return null;
    const body = hit.object.userData.body as number;
    const face = this.meshData[body]?.faceIds[hit.faceIndex];
    return face === undefined ? null : { body, face };
  }

  /** Edge under a pointer event, if any (within a few pixels). */
  private pickEdgeAt(e: PointerEvent): EdgePick | null {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.camera);
    // Threshold in world units scaled with distance so it is ~6px on screen.
    const dist = this.camera.position.distanceTo(this.controls.target);
    const pxWorld = (2 * dist * Math.tan((this.camera.fov * Math.PI) / 360)) / rect.height;
    this.raycaster.params.Line.threshold = pxWorld * 6;
    const hit = this.raycaster.intersectObjects(this.edgeLines.filter((l) => l.visible), false)[0];
    if (!hit || hit.index === undefined) return null;
    const body = hit.object.userData.body as number;
    const segment = Math.floor(hit.index / 2);
    const ef = this.meshData[body]?.edgeFaces;
    if (!ef) return null;
    return { body, segment, faces: [ef[2 * segment]!, ef[2 * segment + 1]!] };
  }

  /** Highlights edges: all display segments of the given body between the given face pairs. */
  setSelectedEdges(picks: { body: number; faces: [number, number] }[]): void {
    this.showEdges(this.edgeHighlight, picks, FACE_SELECTED);
  }

  private showEdges(group: THREE.Group, picks: { body: number; faces: [number, number] }[], color: number): void {
    this.clear(group);
    const pts: number[] = [];
    for (const p of picks) {
      const m = this.meshData[p.body];
      if (!m) continue;
      // Match by surface: every segment between the same two surfaces is
      // one edge to the user (the rim of a faceted cylinder, say).
      const sf = (f: number) => m.faceSurfaces[f] ?? -1;
      const [pa, pb] = [sf(p.faces[0]), sf(p.faces[1])];
      for (let s = 0; s < m.edgeFaces.length / 2; s++) {
        const a = sf(m.edgeFaces[2 * s]!), b = sf(m.edgeFaces[2 * s + 1]!);
        const match = (a === pa && b === pb) || (a === pb && b === pa);
        if (match) for (let k = 0; k < 6; k++) pts.push(m.edges[6 * s + k]!);
      }
    }
    if (pts.length === 0) return;
    const geom = new THREE.BufferGeometry();
    geom.setAttribute("position", new THREE.Float32BufferAttribute(pts, 3));
    const mat = new THREE.LineBasicMaterial({ color, depthTest: false });
    group.add(new THREE.LineSegments(geom, mat));
    // Fatten the highlight with points at segment ends so it reads at any zoom.
    const pmat = new THREE.PointsMaterial({ color, size: 5, sizeAttenuation: false, depthTest: false });
    group.add(new THREE.Points(geom, pmat));
  }

  /** Draws connector frames as axis triads (x red, y green, z blue). */
  setFrames(frames: PlaneFrame[]): void {
    this.clear(this.frames);
    if (frames.length === 0) return;
    const box = new THREE.Box3().expandByObject(this.bodies);
    const len = box.isEmpty() ? 5 : Math.max(2, box.getSize(new THREE.Vector3()).length() * 0.06);
    for (const f of frames) {
      const o = new THREE.Vector3(f.origin.x, f.origin.y, f.origin.z);
      for (const [axis, color] of [[f.x_axis, 0xff5555], [f.y_axis, 0x55dd55], [f.normal, 0x5599ff]] as [Vec3, number][]) {
        const geom = new THREE.BufferGeometry().setFromPoints([o, o.clone().add(new THREE.Vector3(axis.x, axis.y, axis.z).multiplyScalar(len))]);
        this.frames.add(new THREE.Line(geom, new THREE.LineBasicMaterial({ color, depthTest: false, linewidth: 2 })));
      }
      const dot = new THREE.Points(new THREE.BufferGeometry().setFromPoints([o]), new THREE.PointsMaterial({ color: 0xffffff, size: 8, sizeAttenuation: false, depthTest: false }));
      this.frames.add(dot);
    }
  }

  /** Tints whole bodies (by index) as selected; others keep the body colour. */
  setSelectedBodies(indices: ReadonlySet<number>): void {
    for (const m of this.meshes) {
      const i = m.userData.body as number;
      m.material = indices.has(i) ? this.selectedBodyMaterial : this.bodyMaterial;
    }
  }

  /** Highlights a face (or clears the highlight with `null`). */
  setSelectedFace(pick: FacePick | null): void {
    this.showFace(this.highlight, pick, FACE_SELECTED, 0.55);
  }

  private showFace(group: THREE.Group, pick: FacePick | null, color: number, opacity: number): void {
    this.clear(group);
    if (!pick) return;
    const m = this.meshData[pick.body];
    if (!m) return;
    const tris: number[] = [];
    for (let t = 0; t < m.faceIds.length; t++) {
      if (m.faceIds[t] === pick.face) tris.push(m.indices[3 * t]!, m.indices[3 * t + 1]!, m.indices[3 * t + 2]!);
    }
    if (tris.length === 0) return;
    const geom = new THREE.BufferGeometry();
    geom.setAttribute("position", new THREE.BufferAttribute(m.positions, 3));
    geom.setIndex(tris);
    const mat = new THREE.MeshBasicMaterial({ color, transparent: true, opacity, depthTest: true, polygonOffset: true, polygonOffsetFactor: -2, polygonOffsetUnits: -2, side: THREE.DoubleSide });
    group.add(new THREE.Mesh(geom, mat));
  }

  /** Frames the camera on everything currently shown. */
  /** World bounds and centroid of one face's triangles, for detail views. */
  faceBounds(body: number, face: number): { min: Vec3; max: Vec3; centre: Vec3 } | null {
    const data = this.meshData[body];
    if (!data) return null;
    let min = { x: Infinity, y: Infinity, z: Infinity }, max = { x: -Infinity, y: -Infinity, z: -Infinity };
    let sum = { x: 0, y: 0, z: 0 }, count = 0;
    for (let t = 0; t < data.faceIds.length; t++) {
      if (data.faceIds[t] !== face) continue;
      for (let k = 0; k < 3; k++) {
        const vi = data.indices[3 * t + k]!;
        const p = { x: data.positions[3 * vi]!, y: data.positions[3 * vi + 1]!, z: data.positions[3 * vi + 2]! };
        min = { x: Math.min(min.x, p.x), y: Math.min(min.y, p.y), z: Math.min(min.z, p.z) };
        max = { x: Math.max(max.x, p.x), y: Math.max(max.y, p.y), z: Math.max(max.z, p.z) };
        sum = { x: sum.x + p.x, y: sum.y + p.y, z: sum.z + p.z };
        count++;
      }
    }
    if (count === 0) return null;
    return { min, max, centre: { x: sum.x / count, y: sum.y / count, z: sum.z / count } };
  }

  /** The current view as a PNG (rendered fresh, so the buffer is not stale). */
  /** A PNG of the current view, at the canvas size or scaled to fit `size` (cover, centred). */
  snapshot(size?: { width: number; height: number }): Promise<Blob> {
    this.renderer.render(this.scene, this.camera);
    const source = this.renderer.domElement;
    let canvas: HTMLCanvasElement = source;
    if (size) {
      canvas = document.createElement("canvas");
      canvas.width = size.width;
      canvas.height = size.height;
      const ctx = canvas.getContext("2d");
      if (!ctx) return Promise.reject(new Error("could not scale the view"));
      const scale = Math.max(size.width / source.width, size.height / source.height);
      const w = source.width * scale, h = source.height * scale;
      ctx.drawImage(source, (size.width - w) / 2, (size.height - h) / 2, w, h);
    }
    return new Promise((resolve, reject) => {
      canvas.toBlob((blob) => (blob ? resolve(blob) : reject(new Error("could not capture the view"))), "image/png");
    });
  }

  fitAll(): void {
    const box = new THREE.Box3();
    box.expandByObject(this.bodies);
    box.expandByObject(this.sketches);
    if (box.isEmpty()) return;
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3()).length();
    const dist = Math.max(size, 10) / (2 * Math.tan((this.camera.fov * Math.PI) / 360));
    const dir = new THREE.Vector3(0.6, -0.7, 0.5).normalize();
    this.camera.position.copy(center).addScaledVector(dir, dist * 1.3);
    this.controls.target.copy(center);
    this.controls.update();
  }

  private clear(group: THREE.Group): void {
    for (const child of [...group.children]) {
      group.remove(child);
      const obj = child as THREE.Mesh;
      obj.geometry?.dispose();
      const css = child as CSS2DObject;
      if (css.element && css.element.parentNode) css.element.parentNode.removeChild(css.element);
    }
  }
}
