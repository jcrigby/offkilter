// three.js viewport: bodies, sketch curves, grid and standard planes.

import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { BodyMesh, PlaneFrame, SketchResult, Vec2, Vec3 } from "./kernel";

const BODY_COLOR = 0x8fa8c8;
const SKETCH_COLOR = 0x4ea1ff;
const SKETCH_SELECTED = 0xffd166;
const FACE_SELECTED = 0xffd166;
const FACE_HOVER = 0x4ea1ff;

/** A picked face: body index and face index within that body's solid. */
export type FacePick = { body: number; face: number };

/** Pointer events routed to a sketch tool while sketch mode is active. */
export interface PointerHandler {
  down(e: PointerEvent): void;
  move(e: PointerEvent): void;
  up(e: PointerEvent): void;
  cancel(): void;
}

const SKETCH_ENTITY_SELECTED = 0xff7b3a;

export class Viewer {
  private renderer: THREE.WebGLRenderer;
  private scene = new THREE.Scene();
  private camera: THREE.PerspectiveCamera;
  private controls: OrbitControls;
  private bodies = new THREE.Group();
  private sketches = new THREE.Group();
  // Normals come from the kernel (analytic on curved surfaces), so no flat shading.
  private bodyMaterial = new THREE.MeshStandardMaterial({ color: BODY_COLOR, metalness: 0.1, roughness: 0.6 });
  private edgeMaterial = new THREE.LineBasicMaterial({ color: 0x1b1d21 });
  private meshes: THREE.Mesh[] = [];
  private meshData: BodyMesh[] = [];
  private highlight = new THREE.Group();
  private hover = new THREE.Group();
  private raycaster = new THREE.Raycaster();
  private pointerDown: { x: number; y: number } | null = null;
  /** Called when the user clicks a face (or empty space with `null`). */
  onPick: ((pick: FacePick | null) => void) | null = null;
  /** When true, hovering previews faces; used while a panel waits for a face. */
  pickMode = false;
  /** Receives left-button pointer events instead of picking while set. */
  pointerHandler: PointerHandler | null = null;
  private preview = new THREE.Group();
  private previewMaterial = new THREE.LineBasicMaterial({ color: SKETCH_SELECTED, depthTest: false });

  constructor(private container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.renderer.setClearColor(0x1b1d21);
    container.appendChild(this.renderer.domElement);

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

    this.scene.add(this.preview);
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
      this.onPick?.(this.pickAt(e));
    });
    el.addEventListener("pointermove", (e) => {
      if (this.pointerHandler) {
        this.pointerHandler.move(e);
        return;
      }
      if (!this.pickMode || this.pointerDown) return;
      this.showFace(this.hover, this.pickAt(e), FACE_HOVER, 0.35);
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
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }

  private animate = (): void => {
    requestAnimationFrame(this.animate);
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
  };

  setBodies(meshes: BodyMesh[]): void {
    this.clear(this.bodies);
    this.clear(this.highlight);
    this.clear(this.hover);
    this.meshes = [];
    this.meshData = meshes;
    for (const m of meshes) {
      const geom = new THREE.BufferGeometry();
      geom.setAttribute("position", new THREE.BufferAttribute(m.positions, 3));
      geom.setAttribute("normal", new THREE.BufferAttribute(m.normals, 3));
      geom.setIndex(new THREE.BufferAttribute(m.indices, 1));
      const mesh = new THREE.Mesh(geom, this.bodyMaterial);
      mesh.userData.body = this.meshes.length;
      this.meshes.push(mesh);
      this.bodies.add(mesh);
      if (m.edges.length > 0) {
        const eg = new THREE.BufferGeometry();
        eg.setAttribute("position", new THREE.BufferAttribute(m.edges, 3));
        this.bodies.add(new THREE.LineSegments(eg, this.edgeMaterial));
      }
    }
  }

  setSketches(sketches: Record<string, SketchResult>, selected: number | null, selectedEntities: ReadonlySet<number> = new Set()): void {
    this.clear(this.sketches);
    for (const [id, sk] of Object.entries(sketches)) {
      const isSelected = Number(id) === selected;
      const color = isSelected ? SKETCH_SELECTED : SKETCH_COLOR;
      const lineMat = new THREE.LineBasicMaterial({ color, depthTest: !isSelected });
      const pointMat = new THREE.PointsMaterial({ color, size: isSelected ? 7 : 4, sizeAttenuation: false, depthTest: !isSelected });
      const selLineMat = new THREE.LineBasicMaterial({ color: SKETCH_ENTITY_SELECTED, depthTest: false, linewidth: 2 });
      const selPointMat = new THREE.PointsMaterial({ color: SKETCH_ENTITY_SELECTED, size: 10, sizeAttenuation: false, depthTest: false });
      for (const c of sk.curves) {
        const pts = c.points.map((p: Vec3) => new THREE.Vector3(p.x, p.y, p.z));
        const sel = isSelected && selectedEntities.has(c.entity);
        if (c.kind === "point") {
          this.sketches.add(new THREE.Points(new THREE.BufferGeometry().setFromPoints(pts), sel ? selPointMat : pointMat));
        } else {
          this.sketches.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), sel ? selLineMat : lineMat));
        }
      }
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

  /** Face under a pointer event, if any. */
  private pickAt(e: PointerEvent): FacePick | null {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.camera);
    const hit = this.raycaster.intersectObjects(this.meshes, false)[0];
    if (!hit || hit.faceIndex === undefined || hit.faceIndex === null) return null;
    const body = hit.object.userData.body as number;
    const face = this.meshData[body]?.faceIds[hit.faceIndex];
    return face === undefined ? null : { body, face };
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
    }
  }
}
