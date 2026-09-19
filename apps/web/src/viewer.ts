// three.js viewport: bodies, sketch curves, grid and standard planes.

import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { BodyMesh, SketchResult, Vec3 } from "./kernel";

const BODY_COLOR = 0x8fa8c8;
const SKETCH_COLOR = 0x4ea1ff;
const SKETCH_SELECTED = 0xffd166;
const FACE_SELECTED = 0xffd166;
const FACE_HOVER = 0x4ea1ff;

/** A picked face: body index and face index within that body's solid. */
export type FacePick = { body: number; face: number };

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

    const el = this.renderer.domElement;
    el.addEventListener("pointerdown", (e) => {
      this.pointerDown = { x: e.clientX, y: e.clientY };
    });
    el.addEventListener("pointerup", (e) => {
      const down = this.pointerDown;
      this.pointerDown = null;
      if (!down || e.button !== 0) return;
      if (Math.hypot(e.clientX - down.x, e.clientY - down.y) > 4) return; // it was a drag
      this.onPick?.(this.pickAt(e));
    });
    el.addEventListener("pointermove", (e) => {
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

  setSketches(sketches: Record<string, SketchResult>, selected: number | null): void {
    this.clear(this.sketches);
    for (const [id, sk] of Object.entries(sketches)) {
      const isSelected = Number(id) === selected;
      const color = isSelected ? SKETCH_SELECTED : SKETCH_COLOR;
      const lineMat = new THREE.LineBasicMaterial({ color, depthTest: !isSelected });
      const pointMat = new THREE.PointsMaterial({ color, size: isSelected ? 6 : 4, sizeAttenuation: false, depthTest: !isSelected });
      for (const c of sk.curves) {
        const pts = c.points.map((p: Vec3) => new THREE.Vector3(p.x, p.y, p.z));
        if (c.kind === "point") {
          this.sketches.add(new THREE.Points(new THREE.BufferGeometry().setFromPoints(pts), pointMat));
        } else {
          this.sketches.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), lineMat));
        }
      }
    }
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
